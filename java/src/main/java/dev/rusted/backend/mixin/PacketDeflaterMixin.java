package dev.rusted.backend.mixin;

import dev.rusted.backend.NativeCodec;
import io.netty.buffer.ByteBuf;
import io.netty.channel.ChannelHandlerContext;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Production Fabric intermediary namespace for EXACTLY Minecraft 1.20.4.
 * Source: Yarn 1.20.4 commit f7f320f7305c92be62c1cf96679b03c29bb3b87d.
 * Inject the documented erased bridge descriptor, not a guessed typed overload.
 * This hook still requires validation against the actual transformed game.
 */
@Mixin(targets = "net.minecraft.class_2534", remap = false)
public abstract class PacketDeflaterMixin {
    @Shadow(remap = false) private int field_11636;

    @Inject(
        method = "encode(Lio/netty/channel/ChannelHandlerContext;Ljava/lang/Object;Lio/netty/buffer/ByteBuf;)V",
        at = @At("HEAD"), cancellable = true, require = 1, remap = false
    )
    private void rusted$encode(ChannelHandlerContext context, Object message, ByteBuf output, CallbackInfo callback) {
        if (!NativeCodec.enabled() || !(message instanceof ByteBuf input)) return;
        int length = input.readableBytes();
        if (length < field_11636 || length > NativeCodec.MAX_BYTES) return;
        byte[] bytes = new byte[length];
        input.getBytes(input.readerIndex(), bytes); // No indices/output touched until native succeeds.
        byte[] compressed = NativeCodec.tryCompress(bytes);
        if (compressed == null) return;
        output.ensureWritable(5 + compressed.length);
        int remaining = length;
        do {
            int current = remaining & 0x7f;
            remaining >>>= 7;
            output.writeByte(remaining == 0 ? current : current | 0x80);
        } while (remaining != 0);
        output.writeBytes(compressed);
        input.skipBytes(length);
        callback.cancel();
    }
}
