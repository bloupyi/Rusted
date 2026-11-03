// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.fabricmc.loader.api;
public interface Version extends Comparable<Version> {
    String getFriendlyString();
    static Version parse(String value) throws VersionParsingException {
        return dev.rusted.compat.Versions.parse(value);
    }
}
