// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.fabricmc.loader.api.metadata;
public interface ModMetadata {
    String getId();
    String getName();
    net.fabricmc.loader.api.Version getVersion();
}
