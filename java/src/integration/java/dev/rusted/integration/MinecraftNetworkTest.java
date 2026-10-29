package dev.rusted.integration;

import dev.rusted.backend.NativeCodec;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import io.netty.channel.ChannelHandler;
import io.netty.channel.embedded.EmbeddedChannel;
import net.fabricmc.loader.api.entrypoint.PreLaunchEntrypoint;

import java.lang.reflect.Constructor;
import java.io.BufferedOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Random;
import java.util.concurrent.Callable;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.TimeUnit;

/**
 * Separate TEST MOD: Fabric loads the actual remapped Minecraft classes and our
 * actual mixin, then tests them before starting the server. No stubs, world,
 * account authentication, EULA acceptance, or modified server access controls.
 */
public final class MinecraftNetworkTest implements PreLaunchEntrypoint {
    private static final AtomicInteger CASES = new AtomicInteger();
    private static final AtomicInteger NATIVE_EXPECTED = new AtomicInteger();
    private static Constructor<?> encoder;
    private static Constructor<?> decoder;

    private static int readLength(InputStream input) throws Exception {
        int length = 0;
        for (int i = 0; i < 3; i++) {
            int value = input.read(); check(value >= 0, "EOF inside TCP frame length");
            length |= (value & 127) << (7 * i);
            if ((value & 128) == 0) {
                check(length > 0 && length <= 65536, "TCP test frame exceeds bounded length");
                return length;
            }
        }
        throw new AssertionError("Oversized TCP test frame length");
    }
    private static void writeLength(OutputStream output, int length) throws Exception {
        do { int b = length & 127; length >>>= 7; output.write(length == 0 ? b : b | 128); } while (length != 0);
    }
    private static void tcpRoundtrip(boolean expectNative) throws Exception {
        final int packets = 24;
        byte[][] inputs = new byte[packets][];
        for (int i = 0; i < packets; i++) {
            inputs[i] = new byte[513 + i * 37]; new Random(i + 123).nextBytes(inputs[i]);
        }
        var worker = Executors.newSingleThreadExecutor();
        try (var listener = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
            listener.setSoTimeout(10000);
            var receiver = worker.submit(() -> {
                var channel = new EmbeddedChannel((ChannelHandler) decoder.newInstance(256, true));
                try (Socket socket = listener.accept()) {
                    socket.setSoTimeout(10000);
                    var stream = new DataInputStream(socket.getInputStream());
                    for (byte[] expected : inputs) {
                        int length = readLength(stream); byte[] bytes = new byte[length]; stream.readFully(bytes);
                        check(channel.writeInbound(Unpooled.wrappedBuffer(bytes)), "No decoded TCP payload");
                        ByteBuf restored = channel.readInbound();
                        try {
                            byte[] actual = new byte[restored.readableBytes()]; restored.readBytes(actual);
                            check(Arrays.equals(expected, actual), "TCP payload corrupted or reordered");
                        } finally { restored.release(); }
                    }
                    new DataOutputStream(socket.getOutputStream()).writeInt(packets);
                    return packets;
                } finally { channel.finishAndReleaseAll(); }
            });
            var channel = new EmbeddedChannel((ChannelHandler) encoder.newInstance(256));
            try (var socket = new Socket()) {
                socket.connect(new InetSocketAddress(listener.getInetAddress(), listener.getLocalPort()), 10000);
                socket.setSoTimeout(10000);
                var stream = new BufferedOutputStream(socket.getOutputStream(), 257);
                for (byte[] input : inputs) {
                    check(channel.writeOutbound(Unpooled.wrappedBuffer(input.clone())), "No encoded TCP payload");
                    ByteBuf wire = channel.readOutbound();
                    try {
                        byte[] bytes = new byte[wire.readableBytes()]; wire.readBytes(bytes); writeLength(stream, bytes.length);
                        // Fragment application writes and cross packet/buffer boundaries.
                        for (int offset = 0; offset < bytes.length; offset += 17) stream.write(bytes, offset, Math.min(17, bytes.length - offset));
                    } finally { wire.release(); }
                    if (expectNative) NATIVE_EXPECTED.incrementAndGet();
                }
                stream.flush();
                check(new DataInputStream(socket.getInputStream()).readInt() == packets, "Wrong TCP acknowledgement");
                check(receiver.get(10, TimeUnit.SECONDS) == packets, "TCP receiver did not finish");
                CASES.addAndGet(packets);
            } finally { channel.finishAndReleaseAll(); }
        } finally { worker.shutdownNow(); }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
    private static int varint(ByteBuf frame) {
        int value = 0;
        for (int i = 0; i < 5; i++) {
            int b = frame.readUnsignedByte();
            value |= (b & 127) << (7 * i);
            if ((b & 128) == 0) return value;
        }
        throw new AssertionError("Invalid wire VarInt");
    }
    private static void roundtrip(int threshold, byte[] data, boolean expectNative) throws Exception {
        var outbound = new EmbeddedChannel((ChannelHandler) encoder.newInstance(threshold));
        var inbound = new EmbeddedChannel((ChannelHandler) decoder.newInstance(threshold, true));
        ByteBuf input = Unpooled.wrappedBuffer(data.clone());
        input.retain();
        ByteBuf wire = null;
        ByteBuf result = null;
        try {
            check(outbound.writeOutbound(input), "Minecraft encoder produced no output");
            check(input.readableBytes() == 0, "Encoder failed to consume the source buffer");
            wire = outbound.readOutbound();
            check(wire != null && outbound.readOutbound() == null, "Unexpected encoder output count");
            int declared = varint(wire.duplicate());
            check(declared == (data.length < threshold ? 0 : data.length), "Wrong uncompressed-size framing");
            check(inbound.writeInbound(wire.retainedDuplicate()), "Original Minecraft decoder produced no output");
            result = inbound.readInbound();
            check(result != null && result.readableBytes() == data.length, "Original decoder length differs");
            byte[] restored = new byte[data.length]; result.readBytes(restored);
            check(Arrays.equals(data, restored), "Original Minecraft decoded different payload bytes");
            check(inbound.readInbound() == null, "Unexpected trailing decoded packet");
            CASES.incrementAndGet();
            if (expectNative && data.length >= threshold) NATIVE_EXPECTED.incrementAndGet();
        } finally {
            if (result != null) result.release();
            if (wire != null) wire.release();
            input.release();
            outbound.finishAndReleaseAll();
            inbound.finishAndReleaseAll();
        }
    }

    @Override public void onPreLaunch() {
        Path report = Path.of(System.getProperty("rusted.integration.report"));
        String mode = System.getProperty("rusted.integration.mode", "enabled");
        try {
            ClassLoader loader = getClass().getClassLoader();
            // Verified Yarn intermediary classes and constructor signatures for 1.20.4.
            Class<?> encoderClass = Class.forName("net.minecraft.class_2534", true, loader);
            Class<?> decoderClass = Class.forName("net.minecraft.class_2532", true, loader);
            encoder = encoderClass.getDeclaredConstructor(int.class);
            decoder = decoderClass.getDeclaredConstructor(int.class, boolean.class);
            encoder.setAccessible(true); decoder.setAccessible(true);
            boolean enabled = mode.equals("enabled");
            check(mode.equals("enabled") || mode.equals("disabled") || mode.equals("missing"), "Unknown test mode");
            check(NativeCodec.available() != mode.equals("missing"), "Unexpected native availability: " + NativeCodec.loadError());

            for (int threshold : new int[]{0, 1, 256, 1024, 2097152}) {
                for (int size : new int[]{0, 1, 255, 256, 257, 8192, 65536, 1048576}) {
                    // An empty compressed packet at threshold zero is not a real protocol packet.
                    if (size == 0 && threshold == 0) continue;
                    for (boolean random : new boolean[]{false, true}) {
                        byte[] bytes = new byte[size]; if (random) new Random(42 + size).nextBytes(bytes);
                        System.setProperty("rusted.enabled", "false");
                        roundtrip(threshold, bytes, false); // Real vanilla encoder baseline.
                        System.setProperty("rusted.enabled", Boolean.toString(!mode.equals("disabled")));
                        roundtrip(threshold, bytes, enabled); // Same game class, actual injected hook.
                    }
                }
            }
            byte[] large = new byte[NativeCodec.MAX_BYTES]; new Random(7).nextBytes(large);
            roundtrip(256, large, enabled);
            // Changing the real handler threshold must also be observed by the shadow field.
            var setter = encoderClass.getDeclaredMethod("method_10742", int.class);
            Object configurable = encoder.newInstance(256);
            setter.invoke(configurable, 1024);
            var channel = new EmbeddedChannel((ChannelHandler) configurable);
            try {
                channel.writeOutbound(Unpooled.wrappedBuffer(new byte[512]));
                ByteBuf frame = channel.readOutbound();
                try { check(varint(frame) == 0, "Runtime threshold update was ignored"); }
                finally { frame.release(); }
            } finally { channel.finishAndReleaseAll(); }

            var workers = Executors.newFixedThreadPool(4);
            try {
                var jobs = new java.util.ArrayList<Callable<Void>>();
                for (int thread = 0; thread < 4; thread++) {
                    final int id = thread;
                    jobs.add(() -> {
                        for (int i = 0; i < 30; i++) {
                            byte[] bytes = new byte[1024 + id + i]; new Random(i + id * 100).nextBytes(bytes);
                            roundtrip(256, bytes, enabled);
                        }
                        return null;
                    });
                }
                for (var future : workers.invokeAll(jobs)) future.get();
            } finally { workers.shutdownNow(); }
            tcpRoundtrip(enabled);
            check(NativeCodec.encodedPackets() == NATIVE_EXPECTED.get(), "Actual mixin/native counter differs: " + NativeCodec.encodedPackets() + " vs " + NATIVE_EXPECTED.get());
            check(!enabled || NATIVE_EXPECTED.get() > 0, "No actual game call reached Rust");
            Files.createDirectories(report.toAbsolutePath().getParent());
            String json = "{\"passed\":true,\"minecraft\":\"1.20.4\",\"fabric_loader\":\"0.15.11\",\"mode\":\"" + mode
                + "\",\"real_game_codec_cases\":" + CASES.get() + ",\"native_calls\":" + NativeCodec.encodedPackets()
                + ",\"expected_native_calls\":" + NATIVE_EXPECTED.get() + ",\"threshold_update\":true,\"tcp_ordered_packets\":24,\"full_gameplay_tested\":false,\"authenticated_multiplayer_tested\":false}";
            Files.writeString(report, json);
            System.out.println("RUSTED_MINECRAFT_NETWORK_PASS " + json);
            System.exit(0); // Test launcher stops before Minecraft's server main/world.
        } catch (Throwable failure) {
            failure.printStackTrace();
            try { Files.createDirectories(report.toAbsolutePath().getParent()); Files.writeString(report, "{\"passed\":false}"); }
            catch (Exception ignored) { }
            System.exit(1);
        }
    }
}
