//! Microsoft device login with the launcher's own registered public client id.
//! Passwords are entered in the system browser; refresh/session tokens use Windows DPAPI.
use crate::{
    download,
    model::{atomic_json, now, Store},
    Event, Result, Task,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub expires: u64,
}
#[derive(Serialize, Deserialize)]
pub struct Session {
    pub account: Account,
    pub access_token: String,
    pub refresh_token: String,
    pub client_id: String,
    pub xuid: Option<String>,
}
fn value(v: &Value, key: &str) -> Result<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or("Reponse d'authentification incomplete.".into())
}
fn body(response: ureq::Response) -> Result<Value> {
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Reponse d'authentification interrompue.")?;
    if bytes.len() > 1024 * 1024 {
        return Err("Reponse d'authentification trop volumineuse.".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Reponse d'authentification invalide.".into())
}
fn post(url: &str, v: Value) -> Result<Value> {
    body(
        download::agent()
            .post(url)
            .set("Accept", "application/json")
            .send_json(v)
            .map_err(|_| {
                "Authentification Xbox/Minecraft refusee. Verifier le compte et son profil Xbox."
            })?,
    )
}
fn exchange(ms: &str, refresh: String, client_id: String, task: &Task) -> Result<Session> {
    task.check()?;
    task.status("Connexion Xbox Live");
    let x = post(
        "https://user.auth.xboxlive.com/user/authenticate",
        json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={ms}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}),
    )?;
    task.check()?;
    let token = value(&x, "Token")?;
    let x = post(
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[token]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}),
    )?;
    let hash = x
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(Value::as_str)
        .ok_or("Profil Xbox incomplet.")?;
    let xuid = x
        .pointer("/DisplayClaims/xui/0/xid")
        .and_then(Value::as_str)
        .map(str::to_owned);
    task.check()?;
    task.status("Verification du compte Minecraft Java");
    let m = post(
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        json!({"identityToken":format!("XBL3.0 x={hash};{}",value(&x,"Token")?)}),
    )?;
    let token = value(&m, "access_token")?;
    let entitlement = body(
        download::agent()
            .get("https://api.minecraftservices.com/entitlements/mcstore")
            .set("Authorization", &format!("Bearer {token}"))
            .call()
            .map_err(|_| "Impossible de verifier l'acces a Minecraft Java.")?,
    )?;
    if entitlement["items"].as_array().is_none_or(|a| a.is_empty()) {
        return Err("Ce compte ne donne pas acces a Minecraft Java Edition.".into());
    }
    let profile = body(
        download::agent()
            .get("https://api.minecraftservices.com/minecraft/profile")
            .set("Authorization", &format!("Bearer {token}"))
            .call()
            .map_err(|_| "Profil Minecraft Java absent. Verifier le pseudo sur minecraft.net.")?,
    )?;
    let id = value(&profile, "id")?;
    let name = value(&profile, "name")?;
    if id.len() != 32
        || !id.bytes().all(|b| b.is_ascii_hexdigit())
        || name.len() > 16
        || name.is_empty()
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("Profil Minecraft invalide.".into());
    }
    let expires = now() + m["expires_in"].as_u64().unwrap_or(3600).clamp(60, 86400);
    Ok(Session {
        account: Account { id, name, expires },
        access_token: token,
        refresh_token: refresh,
        client_id,
        xuid,
    })
}
pub fn sign_in(store: &Store, client_id: &str, task: &Task) -> Result<Account> {
    if client_id.len() != 36
        || !client_id
            .bytes()
            .all(|c| c.is_ascii_hexdigit() || c == b'-')
    {
        return Err("Renseigner l'identifiant d'application Microsoft propre a Rusted dans les parametres. Aucun identifiant d'un autre lanceur n'est reutilise.".into());
    }
    task.status("Demande de connexion Microsoft");
    let d=body(download::agent().post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode").send_form(&[("client_id",client_id),("scope","XboxLive.signin offline_access")]).map_err(|_|"Demande Microsoft refusee. L'application doit autoriser les clients publics et les comptes personnels.")?)?;
    let device = value(&d, "device_code")?;
    let code = value(&d, "user_code")?;
    let uri = value(&d, "verification_uri")?;
    let parsed =
        url::Url::parse(&uri).map_err(|_| "Adresse de verification Microsoft invalide.")?;
    if parsed.scheme() != "https"
        || !matches!(
            parsed.host_str(),
            Some("microsoft.com" | "www.microsoft.com" | "login.microsoftonline.com")
        )
    {
        return Err("Adresse de verification Microsoft inattendue.".into());
    }
    task.send(Event::DeviceCode { uri, code });
    let deadline =
        Instant::now() + Duration::from_secs(d["expires_in"].as_u64().unwrap_or(900).min(1800));
    let mut interval = d["interval"].as_u64().unwrap_or(5).clamp(1, 30);
    loop {
        for _ in 0..interval {
            task.check()?;
            std::thread::sleep(Duration::from_secs(1));
        }
        if Instant::now() >= deadline {
            return Err("Le code Microsoft a expire. Recommencer la connexion.".into());
        }
        let request = download::agent()
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .send_form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", &device),
            ]);
        match request {
            Ok(r) => {
                let v = body(r)?;
                let session = exchange(
                    &value(&v, "access_token")?,
                    value(&v, "refresh_token")?,
                    client_id.into(),
                    task,
                )?;
                task.check()?;
                save(store, &session)?;
                return Ok(session.account);
            }
            Err(ureq::Error::Status(_, r)) => {
                let v = body(r)?;
                match v["error"].as_str() {
                    Some("authorization_pending") => {}
                    Some("slow_down") => interval = (interval + 5).min(30),
                    Some("authorization_declined") => {
                        return Err("Connexion annulee dans le navigateur.".into())
                    }
                    _ => return Err("Connexion Microsoft refusee ou expiree.".into()),
                }
            }
            Err(_) => return Err("Connexion Microsoft interrompue.".into()),
        }
    }
}
pub fn list(store: &Store) -> Result<Vec<Account>> {
    let mut a = vec![];
    for e in fs::read_dir(store.root.join("accounts"))
        .map_err(|_| "Comptes inaccessibles.")?
        .flatten()
    {
        if e.path().extension().is_some_and(|s| s == "json") {
            a.push(crate::model::read_json(&e.path())?)
        }
    }
    Ok(a)
}
fn save(store: &Store, session: &Session) -> Result<()> {
    let bytes = serde_json::to_vec(session).map_err(|_| "Session invalide.")?;
    let encrypted = protect(&bytes)?;
    let path = store
        .root
        .join("accounts")
        .join(format!("{}.secret", session.account.id));
    let mut t = tempfile::NamedTempFile::new_in(path.parent().unwrap())
        .map_err(|_| "Stockage de compte inaccessible.")?;
    std::io::Write::write_all(&mut t, &encrypted)
        .map_err(|_| "Enregistrement de session impossible.")?;
    t.persist(path)
        .map_err(|_| "Enregistrement de session impossible.")?;
    atomic_json(
        &store
            .root
            .join("accounts")
            .join(format!("{}.json", session.account.id)),
        &session.account,
    )
}
pub fn session(store: &Store, id: &str, task: &Task) -> Result<Session> {
    crate::model::safe_id(id)?;
    let bytes = fs::read(store.root.join("accounts").join(format!("{id}.secret")))
        .map_err(|_| "Choisir un compte Microsoft connecte.")?;
    let bytes = unprotect(&bytes)?;
    let s: Session = serde_json::from_slice(&bytes)
        .map_err(|_| "Session locale invalide. Reconnecter ce compte.")?;
    if s.account.expires > now() + 120 {
        return Ok(s);
    }
    task.status("Renouvellement de la session Microsoft");
    let v = body(
        download::agent()
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .send_form(&[
                ("client_id", s.client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", s.refresh_token.as_str()),
                ("scope", "XboxLive.signin offline_access"),
            ])
            .map_err(|_| "Session expiree. Reconnecter le compte Microsoft.")?,
    )?;
    let refreshed = exchange(
        &value(&v, "access_token")?,
        v["refresh_token"]
            .as_str()
            .unwrap_or(&s.refresh_token)
            .into(),
        s.client_id,
        task,
    )?;
    save(store, &refreshed)?;
    Ok(refreshed)
}
pub fn forget(store: &Store, id: &str) -> Result<()> {
    crate::model::safe_id(id)?;
    for ext in ["json", "secret"] {
        let p = store.root.join("accounts").join(format!("{id}.{ext}"));
        if p.exists() {
            fs::remove_file(p).map_err(|_| "Suppression du compte local impossible.")?;
        }
    }
    Ok(())
}
#[cfg(windows)]
fn crypt(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes
            .len()
            .try_into()
            .map_err(|_| "Session trop volumineuse.")?,
        pbData: bytes.as_ptr() as _,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: DPAPI operates on the provided input and allocates the result; LocalFree releases it.
    unsafe {
        let ok = if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err("Protection Windows du compte indisponible. Aucune session n'est enregistree en clair.".into());
        }
        let v = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        Ok(v)
    }
}
#[cfg(windows)]
fn protect(v: &[u8]) -> Result<Vec<u8>> {
    crypt(v, true)
}
#[cfg(windows)]
fn unprotect(v: &[u8]) -> Result<Vec<u8>> {
    crypt(v, false)
}
#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>> {
    Err("Le stockage des comptes utilise DPAPI sous Windows.".into())
}
#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Vec<u8>> {
    Err("Ce compte est protege par Windows DPAPI.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_own_client_id_fails_before_network_or_secret_storage() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::open(d.path().into()).unwrap();
        let (tx, _) = std::sync::mpsc::channel();
        let task = Task {
            events: tx,
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        assert!(sign_in(&store, "", &task)
            .err()
            .unwrap()
            .contains("application Microsoft"));
        assert!(list(&store).unwrap().is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip_never_persists_plaintext_tokens() {
        let secret = b"TEST_ONLY_ACCESS_AND_REFRESH_TOKENS";
        let encrypted = protect(secret).unwrap();
        assert!(!encrypted.windows(secret.len()).any(|w| w == secret));
        assert_eq!(unprotect(&encrypted).unwrap(), secret);
    }
}
