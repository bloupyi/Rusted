//! Experimental execution of unchanged Fabric entrypoints inside the native game.
//! The JVM runs mod code only; absent Minecraft APIs fail rather than loading a Java game.
use jni::{
    objects::{GlobalRef, JClass, JObject, JObjectArray, JString, JValue},
    InitArgsBuilder, JNIEnv, JNIVersion, JavaVM, NativeMethod,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

pub type Result<T> = std::result::Result<T, String>;
static JVM: OnceLock<Result<HostVm>> = OnceLock::new();
static OUTPUT: Mutex<Vec<Action>> = Mutex::new(Vec::new());
const API: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/rusted-java-api.jar"));

/// Original compatibility API for developer fixtures; no game or third-party mod classes.
pub fn api_jar() -> &'static [u8] {
    API
}

#[derive(Debug, PartialEq)]
pub enum Action {
    Chat { module: String, text: String },
    Error(String),
}
pub use rusted_platform::java::{discover, inspect, Module};

/// Rust's canonical Windows paths may use an extended prefix that Java paths reject.
fn java_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = path.strip_prefix(r"\\?\") {
            return rest.to_owned();
        }
    }
    path.into_owned()
}

struct HostVm {
    vm: JavaVM,
    host: GlobalRef,
    _library: std::mem::ManuallyDrop<libloading::Library>,
}

/// One JVM per native process. It must not be recreated or unloaded while Java threads exist.
fn vm(java_home: &Path, cache: &Path) -> Result<&'static HostVm> {
    JVM.get_or_init(|| {
        fs::create_dir_all(cache).map_err(|e| e.to_string())?;
        let api_path = cache.join(format!("api-{:x}.jar", Sha256::digest(API)));
        if fs::read(&api_path).ok().as_deref() != Some(API) {
            let mut file = tempfile::NamedTempFile::new_in(cache).map_err(|e| e.to_string())?;
            file.write_all(API).map_err(|e| e.to_string())?;
            file.persist(&api_path).map_err(|e| e.to_string())?;
        }
        let library_path = java_home.join(if cfg!(windows) {
            "bin/server/jvm.dll"
        } else {
            "lib/server/libjvm.so"
        });
        // Pin the library explicitly for the lifetime of the process.
        #[cfg(windows)]
        let library = unsafe {
            use std::os::windows::ffi::OsStrExt;
            #[link(name = "kernel32")]
            extern "system" {
                fn AddDllDirectory(path: *const u16) -> *mut std::ffi::c_void;
            }
            let bin: Vec<u16> = java_home
                .join("bin")
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            if AddDllDirectory(bin.as_ptr()).is_null() {
                return Err("Cannot add the Java runtime DLL directory".into());
            }
            libloading::Library::from(
                libloading::os::windows::Library::load_with_flags(&library_path, 0x1100)
                    .map_err(|e| e.to_string())?,
            )
        };
        #[cfg(not(windows))]
        let library =
            unsafe { libloading::Library::new(&library_path).map_err(|e| e.to_string())? };
        // Keep HotSpot mapped even if API setup fails after JVM creation.
        // A live VM may already own native threads, and cannot be unloaded here.
        let library = std::mem::ManuallyDrop::new(library);
        let args = InitArgsBuilder::new()
            .version(JNIVersion::V8)
            .option("-Djava.class.path=")
            .option("-Xms16m")
            .option("-Xmx256m")
            .option("-Xrs")
            .option("-Djava.awt.headless=true")
            .build()
            .map_err(|e| e.to_string())?;
        let vm = JavaVM::with_libjvm(args, || Ok(library_path)).map_err(|e| e.to_string())?;
        let host = {
            let mut env = vm
                .attach_current_thread_permanently()
                .map_err(|e| e.to_string())?;
            let class = env
                .with_local_frame(32, |env| -> jni::errors::Result<GlobalRef> {
                    let path = env.new_string(java_path(&api_path))?;
                    let file = env.new_object(
                        "java/io/File",
                        "(Ljava/lang/String;)V",
                        &[JValue::Object(&path)],
                    )?;
                    let uri = env
                        .call_method(file, "toURI", "()Ljava/net/URI;", &[])?
                        .l()?;
                    let url = env
                        .call_method(uri, "toURL", "()Ljava/net/URL;", &[])?
                        .l()?;
                    let urls = env.new_object_array(1, "java/net/URL", &url)?;
                    let parent = env
                        .call_static_method(
                            "java/lang/ClassLoader",
                            "getPlatformClassLoader",
                            "()Ljava/lang/ClassLoader;",
                            &[],
                        )?
                        .l()?;
                    let loader = env.new_object(
                        "java/net/URLClassLoader",
                        "([Ljava/net/URL;Ljava/lang/ClassLoader;)V",
                        &[JValue::Object(&urls), JValue::Object(&parent)],
                    )?;
                    let name = env.new_string("dev.rusted.compat.Host")?;
                    let class = env
                        .call_method(
                            loader,
                            "loadClass",
                            "(Ljava/lang/String;)Ljava/lang/Class;",
                            &[JValue::Object(&name)],
                        )?
                        .l()?;
                    env.new_global_ref(class)
                })
                .map_err(|e| java_error(&mut env, e))?;
            let native_class: &JClass = class.as_obj().into();
            env.register_native_methods(
                native_class,
                &[NativeMethod {
                    name: "emitChat".into(),
                    sig: "(Ljava/lang/String;Ljava/lang/String;)V".into(),
                    fn_ptr: emit_chat as *mut std::ffi::c_void,
                }],
            )
            .map_err(|e| java_error(&mut env, e))?;
            class
        };
        Ok(HostVm {
            vm,
            host,
            _library: library,
        })
    })
    .as_ref()
    .map_err(Clone::clone)
}

unsafe extern "system" fn emit_chat(
    mut env: JNIEnv,
    _class: JClass,
    module: JString,
    text: JString,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        for value in [&module, &text] {
            let length = env
                .call_method(value, "length", "()I", &[])
                .and_then(|v| v.i())
                .map_err(|e| e.to_string())?;
            if length > 8192 {
                return Err("Java chat message too large".into());
            }
        }
        let module: String = env.get_string(&module).map_err(|e| e.to_string())?.into();
        let text: String = env.get_string(&text).map_err(|e| e.to_string())?.into();
        let mut output = OUTPUT.lock().unwrap_or_else(|e| e.into_inner());
        if output.len() >= 256 {
            return Err("Java output budget exceeded".into());
        }
        output.push(Action::Chat { module, text });
        Ok(())
    }));
    if !matches!(result, Ok(Ok(()))) {
        let _ = env.throw_new(
            "java/lang/IllegalStateException",
            "Rusted Java output budget exceeded or message invalid",
        );
    }
}

fn strings<'a>(env: &mut JNIEnv<'a>, values: &[String]) -> jni::errors::Result<JObjectArray<'a>> {
    let array = env.new_object_array(values.len() as i32, "java/lang/String", JObject::null())?;
    for (index, value) in values.iter().enumerate() {
        let value = env.new_string(value)?;
        env.set_object_array_element(&array, index as i32, &value)?;
        env.delete_local_ref(value)?;
    }
    Ok(array)
}
fn java_error(env: &mut JNIEnv, error: jni::errors::Error) -> String {
    if !env.exception_check().unwrap_or(false) {
        return error.to_string();
    }
    let result = (|| -> jni::errors::Result<String> {
        let exception = env.exception_occurred()?;
        env.exception_clear()?;
        let message = env
            .call_method(exception, "toString", "()Ljava/lang/String;", &[])?
            .l()?;
        Ok(env.get_string(&JString::from(message))?.into())
    })();
    let _ = env.exception_clear();
    result.unwrap_or_else(|_| error.to_string())
}

pub struct Runtime {
    host: &'static HostVm,
}
impl Runtime {
    pub fn load(
        java_home: &Path,
        game: &Path,
        cache: &Path,
        modules: &[Module],
        libraries: &[PathBuf],
    ) -> Result<Self> {
        let host = vm(java_home, cache)?;
        let native_class: &JClass = host.host.as_obj().into();
        let mut env = host
            .vm
            .attach_current_thread_permanently()
            .map_err(|e| e.to_string())?;
        let result = env.with_local_frame(64, |env| -> jni::errors::Result<()> {
            let directory = game.canonicalize()
                .map_err(|_| jni::errors::Error::NullPtr("game directory"))?;
            let directory = env.new_string(java_path(&directory))?;
            let paths: Vec<_> = modules.iter().map(|m| m.path.clone())
                .chain(libraries.iter().cloned()).map(|p| java_path(&p)).collect();
            let paths = strings(env, &paths)?;
            let ids = strings(env, &modules.iter().map(|m| m.id.clone()).collect::<Vec<_>>())?;
            let names = strings(env, &modules.iter().map(|m| m.name.clone()).collect::<Vec<_>>())?;
            env.call_static_method(native_class, "configure", "(Ljava/lang/String;[Ljava/lang/String;[Ljava/lang/String;[Ljava/lang/String;)V", &[JValue::Object(&directory), JValue::Object(&paths), JValue::Object(&ids), JValue::Object(&names)])?;
            for module in modules {
                for (name, class) in &module.adapters {
                    let id = env.new_string(&module.id)?; let name = env.new_string(name)?; let class = env.new_string(class)?;
                    env.call_static_method(native_class, "adapter", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&id), JValue::Object(&name), JValue::Object(&class)])?;
                    env.delete_local_ref(id)?; env.delete_local_ref(name)?; env.delete_local_ref(class)?;
                }
            }
            for (kind, method) in [("preLaunch", "onPreLaunch"), ("main", "onInitialize"), ("client", "onInitializeClient")] {
                for module in modules {
                    for entry in module.entrypoints.get(kind).into_iter().flatten() {
                        let id = env.new_string(&module.id)?; let class = env.new_string(&entry.value)?; let adapter = env.new_string(&entry.adapter)?; let method = env.new_string(method)?;
                        env.call_static_method(native_class, "initialize", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&id), JValue::Object(&class), JValue::Object(&method), JValue::Object(&adapter)])?;
                        env.delete_local_ref(id)?; env.delete_local_ref(class)?; env.delete_local_ref(method)?; env.delete_local_ref(adapter)?;
                    }
                }
            }
            Ok(())
        });
        result.map_err(|e| java_error(&mut env, e))?;
        Ok(Self { host })
    }
    pub fn tick(&mut self, time: i64, health: f32, present: bool) -> Result<Vec<Action>> {
        let native_class: &JClass = self.host.host.as_obj().into();
        let mut env = self
            .host
            .vm
            .attach_current_thread_permanently()
            .map_err(|e| e.to_string())?;
        let result = env.with_local_frame(64, |env| -> jni::errors::Result<Vec<Action>> {
            env.call_static_method(
                native_class,
                "tick",
                "(JFZ)V",
                &[
                    JValue::Long(time),
                    JValue::Float(health),
                    JValue::Bool(u8::from(present)),
                ],
            )?;
            let errors = env
                .call_static_method(native_class, "drainErrors", "()[Ljava/lang/String;", &[])?
                .l()?;
            let errors = JObjectArray::from(errors);
            let mut actions =
                std::mem::take(&mut *OUTPUT.lock().unwrap_or_else(|e| e.into_inner()));
            for i in 0..env.get_array_length(&errors)? {
                let text = JString::from(env.get_object_array_element(&errors, i)?);
                actions.push(Action::Error(env.get_string(&text)?.into()));
                env.delete_local_ref(text)?;
            }
            Ok(actions)
        });
        result.map_err(|e| java_error(&mut env, e))
    }
}
