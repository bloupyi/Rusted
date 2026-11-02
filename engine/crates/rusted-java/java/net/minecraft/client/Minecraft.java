// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.minecraft.client;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.multiplayer.ClientLevel;
/** Views of Rust's state; this class does not simulate or render Minecraft. */
public final class Minecraft {
    private static final Minecraft INSTANCE = new Minecraft();
    public volatile LocalPlayer player;
    public volatile ClientLevel level;
    private final java.util.concurrent.ConcurrentLinkedQueue<Runnable> tasks = new java.util.concurrent.ConcurrentLinkedQueue<>();
    public boolean isSameThread() { return Thread.currentThread() == dev.rusted.compat.Host.clientThread; }
    public void execute(Runnable action) { if (isSameThread()) action.run(); else tasks.add(action); }
    public void runTasks() { for (int i = 0; i < 256; i++) { Runnable task = tasks.poll(); if (task == null) break; task.run(); } }
    public static Minecraft getInstance() { return INSTANCE; }
    public static void update(long time, float health, boolean present) {
        if (!present) { INSTANCE.player = null; INSTANCE.level = null; return; }
        if (INSTANCE.player == null) INSTANCE.player = new LocalPlayer(health);
        else INSTANCE.player.update(health);
        if (INSTANCE.level == null) INSTANCE.level = new ClientLevel();
        INSTANCE.level.update(time);
    }
}
