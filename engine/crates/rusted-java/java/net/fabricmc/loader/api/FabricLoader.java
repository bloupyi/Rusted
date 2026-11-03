// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.fabricmc.loader.api;
import java.nio.file.Path;
import net.fabricmc.api.EnvType;
import dev.rusted.compat.Host;
/** Implemented public API subset; this is not the Fabric Loader implementation. */
public interface FabricLoader {
    static FabricLoader getInstance() { return Host.LOADER; }
    Path getGameDir();
    Path getConfigDir();
    @Deprecated default java.io.File getGameDirectory() { return getGameDir().toFile(); }
    @Deprecated default java.io.File getConfigDirectory() { return getConfigDir().toFile(); }
    boolean isModLoaded(String id);
    java.util.Optional<ModContainer> getModContainer(String id);
    java.util.Collection<ModContainer> getAllMods();
    boolean isDevelopmentEnvironment();
    EnvType getEnvironmentType();
}
