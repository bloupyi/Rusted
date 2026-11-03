// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.fabricmc.loader.api;
public interface ModContainer {
    net.fabricmc.loader.api.metadata.ModMetadata getMetadata();
    java.util.List<java.nio.file.Path> getRootPaths();
    default java.util.Optional<java.nio.file.Path> findPath(String file) {
        for (var root : getRootPaths()) {
            var path = root.resolve(file);
            if (java.nio.file.Files.exists(path)) return java.util.Optional.of(path);
        }
        return java.util.Optional.empty();
    }
    @Deprecated default java.nio.file.Path getRoot() { return getRootPaths().get(0); }
    @Deprecated default java.nio.file.Path getRootPath() { return getRoot(); }
    @Deprecated default java.nio.file.Path getPath(String file) { return getRoot().resolve(file); }
}
