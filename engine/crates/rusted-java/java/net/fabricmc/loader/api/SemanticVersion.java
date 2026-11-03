// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.fabricmc.loader.api;
public interface SemanticVersion extends Version {
    int COMPONENT_WILDCARD = Integer.MIN_VALUE;
    int getVersionComponentCount();
    int getVersionComponent(int position);
    java.util.Optional<String> getPrereleaseKey();
    java.util.Optional<String> getBuildKey();
    boolean hasWildcard();
    @Deprecated default int compareTo(SemanticVersion other) { return compareTo((Version) other); }
    static SemanticVersion parse(String value) throws VersionParsingException {
        return dev.rusted.compat.Versions.semantic(value);
    }
}
