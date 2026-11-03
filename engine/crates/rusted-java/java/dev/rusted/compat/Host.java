// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package dev.rusted.compat;
import java.nio.file.Path;
import java.net.URL;
import java.net.URLClassLoader;
import java.util.*;
import net.fabricmc.loader.api.*;
import net.fabricmc.api.EnvType;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.Minecraft;

public final class Host {
    public static final ThreadLocal<String> owner = ThreadLocal.withInitial(() -> "java");
    private static final Set<String> failed = new HashSet<>();
    private static final List<String> errors = new ArrayList<>();
    private static final Map<String, LanguageAdapter> adapters = new HashMap<>();
    private static final Map<String, Object> instances = new HashMap<>();
    private static final Map<String, ModContainer> mods = new LinkedHashMap<>();
    private static final Map<Path, java.nio.file.FileSystem> archives = new HashMap<>();
    private static Path gameDir;
    public static volatile Thread clientThread;
    private static URLClassLoader loader;
    public static final FabricLoader LOADER = new FabricLoader() {
        public Path getGameDir() { return gameDir; }
        public Path getConfigDir() { return gameDir.resolve("config"); }
        public boolean isModLoaded(String id) { return mods.containsKey(id); }
        public Optional<ModContainer> getModContainer(String id) { return Optional.ofNullable(mods.get(id)); }
        public Collection<ModContainer> getAllMods() { return List.copyOf(mods.values()); }
        public boolean isDevelopmentEnvironment() { return false; }
        public EnvType getEnvironmentType() { return EnvType.CLIENT; }
    };
    public static void configure(String directory, String[] paths, String[] ids, String[] modNames, String[] versions, String[] origins) throws Exception {
        if (loader != null) throw new IllegalStateException("Java mods require a game restart to reload");
        gameDir = Path.of(directory);
        clientThread = Thread.currentThread();
        clientThread.setName("Rusted client");
        if (ids.length != modNames.length || ids.length != versions.length || ids.length != origins.length)
            throw new IllegalArgumentException("Inconsistent native module metadata");
        addBuiltin("minecraft", "Minecraft (Rusted native)", "26.3");
        addBuiltin("java", "Java", Runtime.version().toString());
        addBuiltin("rusted_java", "Rusted Java bridge", "0.1.0");
        for (int i = 0; i < ids.length; i++) {
            if (mods.containsKey(ids[i])) throw new IllegalArgumentException("Duplicate module id: " + ids[i]);
            mods.put(ids[i], new Container(new Metadata(ids[i], modNames[i], Version.parse(versions[i])), Path.of(origins[i])));
        }
        URL[] urls = new URL[paths.length];
        for (int i = 0; i < paths.length; i++) urls[i] = Path.of(paths[i]).toUri().toURL();
        loader = new URLClassLoader(urls, Host.class.getClassLoader()) {
            @Override protected Class<?> findClass(String name) throws ClassNotFoundException {
                if (name.startsWith("net.minecraft.") || name.startsWith("com.mojang.blaze3d."))
                    throw new ClassNotFoundException("Rusted native API not implemented: " + name);
                return super.findClass(name);
            }
        };
        Thread.currentThread().setContextClassLoader(loader);
    }
    public static final LanguageAdapter DEFAULT_ADAPTER = new LanguageAdapter() {
        public <T> T create(ModContainer mod, String value, Class<T> type) throws LanguageAdapterException {
            try {
                String[] handle = value.split("::", -1);
                if (handle.length > 2 || handle[0].isEmpty() || (handle.length == 2 && handle[1].isEmpty()))
                    throw new IllegalArgumentException("Invalid entrypoint handle: " + value);
                Class<?> entry = Class.forName(handle[0], true, loader == null ? Host.class.getClassLoader() : loader);
                if (handle.length == 1) return type.cast(entry.getDeclaredConstructor().newInstance());
                try {
                    var field = entry.getDeclaredField(handle[1]);
                    if (!java.lang.reflect.Modifier.isStatic(field.getModifiers()) || !type.isAssignableFrom(field.getType()))
                        throw new IllegalArgumentException("Entrypoint field must be static and have the requested type: " + value);
                    return type.cast(field.get(null));
                } catch (NoSuchFieldException absent) { /* A method reference is the other supported form. */ }
                java.lang.reflect.Method method = null;
                for (var candidate : entry.getDeclaredMethods()) {
                    if (!candidate.getName().equals(handle[1])) continue;
                    if (method != null) throw new IllegalArgumentException("Ambiguous overloaded entrypoint: " + value);
                    method = candidate;
                }
                if (method == null) throw new NoSuchMethodException(value);
                var target = java.lang.invoke.MethodHandles.publicLookup().unreflect(method);
                if (!java.lang.reflect.Modifier.isStatic(method.getModifiers()))
                    target = target.bindTo(entry.getDeclaredConstructor().newInstance());
                return java.lang.invoke.MethodHandleProxies.asInterfaceInstance(type, target);
            }
            catch (Throwable error) { throw new LanguageAdapterException(error); }
        }
    };
    public static void adapter(String id, String name, String entry) {
        owner.set(id);
        try {
            if (name.equals("default") || adapters.containsKey(name)) throw new IllegalArgumentException("Duplicate language adapter: " + name);
            LanguageAdapter adapter = LanguageAdapter.class.cast(Class.forName(entry, true, loader).getDeclaredConstructor().newInstance());
            adapters.put(name, adapter);
        } catch (Throwable error) { fail(id, error); }
        finally { owner.remove(); }
    }
    private static void addBuiltin(String id, String name, String version) throws VersionParsingException {
        mods.put(id, new Container(new Metadata(id, name, Version.parse(version)), gameDir));
    }
    private record Metadata(String getId, String getName, Version getVersion) implements net.fabricmc.loader.api.metadata.ModMetadata { }
    private record Container(net.fabricmc.loader.api.metadata.ModMetadata getMetadata, Path origin) implements ModContainer {
        public List<Path> getRootPaths() {
            if (java.nio.file.Files.isDirectory(origin)) return List.of(origin);
            synchronized (archives) {
                try {
                    var archive = archives.get(origin);
                    if (archive == null) {
                        archive = java.nio.file.FileSystems.newFileSystem(origin, Map.of());
                        archives.put(origin, archive);
                    }
                    return List.of(archive.getPath("/"));
                } catch (java.io.IOException error) { throw new java.io.UncheckedIOException(error); }
            }
        }
    }
    public static void initialize(String id, String entry, String method, String adapterName) {
        if (failed(id)) return;
        owner.set(id);
        try {
            Class<?> type = switch (method) {
                case "onPreLaunch" -> net.fabricmc.loader.api.entrypoint.PreLaunchEntrypoint.class;
                case "onInitialize" -> net.fabricmc.api.ModInitializer.class;
                case "onInitializeClient" -> net.fabricmc.api.ClientModInitializer.class;
                default -> throw new IllegalArgumentException("Unsupported initializer");
            };
            LanguageAdapter adapter = adapterName.equals("default") ? DEFAULT_ADAPTER : adapters.get(adapterName);
            if (adapter == null) throw new IllegalStateException("Missing language adapter: " + adapterName);
            String key = id + ":" + method + ":" + adapterName + ":" + entry;
            Object instance = instances.get(key);
            if (instance == null) {
                instance = adapter.create(mods.get(id), entry, type);
                instances.put(key, instance);
            }
            type.getMethod(method).invoke(instance);
        } catch (Throwable error) { fail(id, error); }
        finally { owner.remove(); }
    }
    public static void tick(long time, float health, boolean present) {
        Minecraft.update(time, health, present);
        Minecraft.getInstance().runTasks();
        ClientTickEvents.START_CLIENT_TICK.invoker().onStartTick(Minecraft.getInstance());
        ClientTickEvents.END_CLIENT_TICK.invoker().onEndTick(Minecraft.getInstance());
    }
    public static synchronized boolean failed(String id) { return failed.contains(id); }
    public static synchronized void fail(String id, Throwable error) {
        while (error.getCause() != null && error != error.getCause()) error = error.getCause();
        if (failed.add(id)) {
            String location = error.getStackTrace().length == 0 ? "" : " at " + error.getStackTrace()[0];
            errors.add(id + ": " + error + location);
        }
    }
    public static synchronized String[] drainErrors() {
        String[] result = errors.toArray(String[]::new); errors.clear(); return result;
    }
    public static void chat(String text) { emitChat(owner.get(), text); }
    private static native void emitChat(String mod, String text);
    private Host() { }
}
