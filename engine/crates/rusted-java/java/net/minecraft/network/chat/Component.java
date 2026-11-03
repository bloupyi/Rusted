// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.minecraft.network.chat;
public interface Component {
    static MutableComponent literal(String text) { return new MutableComponent(text); }
    String getString();
}
