// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.fabricmc.loader.api;
public interface LanguageAdapter {
 static LanguageAdapter getDefault() { return dev.rusted.compat.Host.DEFAULT_ADAPTER; }
 <T> T create(ModContainer mod, String value, Class<T> type) throws LanguageAdapterException;
}
