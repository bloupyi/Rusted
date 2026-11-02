// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.minecraft.client.player;
import dev.rusted.compat.Host;
import net.minecraft.network.chat.Component;
public final class LocalPlayer {
    private volatile float health;
    public LocalPlayer(float health) { this.health = health; }
    public void update(float value) { health = value; }
    public float getHealth() { return health; }
    public void sendSystemMessage(Component message) { Host.chat(message.getString()); }
}
