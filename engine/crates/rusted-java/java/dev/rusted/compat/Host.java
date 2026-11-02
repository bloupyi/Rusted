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
    private static final Map<String, String> names = new HashMap<>();
    private static final Set<String> mods = new HashSet<>();
    private static Path gameDir;
    public static volatile Thread clientThread;
    private static URLClassLoader loader;
    public static final FabricLoader LOADER = new FabricLoader() {
        public Path getGameDir() { return gameDir; }
        public Path getConfigDir() { return gameDir.resolve("config"); }
        public boolean isModLoaded(String id) { return mods.contains(id); }
        public boolean isDevelopmentEnvironment() { return false; }
        public EnvType getEnvironmentType() { return EnvType.CLIENT; }
    };
    public static void configure(String directory, String[] paths, String[] ids, String[] modNames) throws Exception {
        if (loader != null) throw new IllegalStateException("Java mods require a game restart to reload");
        gameDir = Path.of(directory);
        clientThread = Thread.currentThread();
        clientThread.setName("Rusted client");
        Collections.addAll(mods, ids);
        for (int i = 0; i < ids.length; i++) names.put(ids[i], modNames[i]);
        Collections.addAll(mods, "minecraft", "fabricloader", "java", "fabric-lifecycle-events-v1");
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
            try { return type.cast(Class.forName(value, true, loader).getDeclaredConstructor().newInstance()); }
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
    private record Metadata(String getId, String getName) implements net.fabricmc.loader.api.metadata.ModMetadata { }
    private record Container(net.fabricmc.loader.api.metadata.ModMetadata getMetadata) implements ModContainer { }
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
                instance = adapter.create(new Container(new Metadata(id, names.getOrDefault(id, id))), entry, type);
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
        if (failed.add(id)) errors.add(id + ": " + error.toString());
    }
    public static synchronized String[] drainErrors() {
        String[] result = errors.toArray(String[]::new); errors.clear(); return result;
    }
    public static void chat(String text) { emitChat(owner.get(), text); }
    private static native void emitChat(String mod, String text);
    private Host() { }
}
