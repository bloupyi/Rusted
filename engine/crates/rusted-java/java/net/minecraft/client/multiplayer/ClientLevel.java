// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.minecraft.client.multiplayer;
import net.minecraft.world.level.storage.LevelData;
public final class ClientLevel extends net.minecraft.world.level.Level {
    public static final class ClientLevelData implements LevelData {
        private volatile long time;
        public long getGameTime() { return time; }
    }
    private final ClientLevelData data = new ClientLevelData();
    public void update(long value) { data.time = value; }
    @Override public ClientLevelData getLevelData() { return data; }
}
