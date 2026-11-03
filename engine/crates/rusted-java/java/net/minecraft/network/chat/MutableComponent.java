// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.minecraft.network.chat;
public final class MutableComponent implements Component {
    private final String text;
    public MutableComponent(String text) { this.text = text; }
    public String getString() { return text; }
}
