package dev.rusted.backend;

import java.io.ByteArrayOutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Random;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.zip.Deflater;
import java.util.zip.Inflater;

/** Real Java <-> native Rust zlib tests. Does NOT establish Minecraft compatibility. */
public final class NativeCompatibilityTest {
    private static int assertions;
    private static void check(boolean condition, String message) {
        assertions++;
        if (!condition) throw new AssertionError(message);
    }
    private static byte[] javaEncode(byte[] bytes, Deflater encoder, byte[] buffer) {
        encoder.reset(); encoder.setInput(bytes); encoder.finish();
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        while (!encoder.finished()) {
            int n = encoder.deflate(buffer);
            if (n == 0 && !encoder.finished()) throw new AssertionError("Java deflater made no progress");
            output.write(buffer, 0, n);
        }
        return output.toByteArray();
    }
    private static byte[] javaDecode(byte[] bytes, int size) throws Exception {
        Inflater decoder = new Inflater();
        try {
            decoder.setInput(bytes);
            byte[] result = new byte[size + 1]; int count = 0;
            while (!decoder.finished()) {
                int n = decoder.inflate(result, count, result.length - count);
                count += n;
                if (n == 0 && !decoder.finished()) throw new AssertionError("Java inflater stalled");
            }
            if (count != size || decoder.getRemaining() != 0) throw new AssertionError("Java decode length/trailing mismatch");
            return Arrays.copyOf(result, count);
        } finally { decoder.end(); }
    }
    private static void expectRejected(byte[] bytes, int size) {
        boolean rejected = false;
        try { NativeCodec.decompressNative(bytes, size); } catch (IllegalArgumentException expected) { rejected = true; }
        check(rejected, "Native decoder accepted malformed input");
    }
    public static void main(String[] args) throws Exception {
        Path report = Path.of(args.length == 0 ? "build/bridge-report.json" : args[0]);
        if (args.length > 1 && (args[1].equals("fallback") || args[1].equals("checksum"))) {
            check(!NativeCodec.available(), "Missing library was reported available");
            check(NativeCodec.tryCompress(new byte[256]) == null, "Missing native must keep Java path");
            check(NativeCodec.encodedPackets() == 0, "Missing native must not count Rust packets");
            if (args[1].equals("checksum")) check(NativeCodec.loadError().contains("SHA-256 mismatch"), "Tampered native checksum was not checked");
            Files.createDirectories(report.toAbsolutePath().getParent());
            Files.writeString(report, "{\"scenario\":\"native-load-fallback-" + args[1] + "\",\"assertions\":" + assertions + ",\"passed\":true}");
            System.out.println("PASS: " + args[1] + " keeps Java backend"); return;
        }
        check(NativeCodec.available(), "JNI unavailable: " + NativeCodec.loadError());
        check(NativeCodec.abiVersion() == 1, "ABI mismatch");
        System.setProperty("rusted.enabled", "false");
        check(NativeCodec.tryCompress(new byte[512]) == null, "Disabled mod must keep Java encode");
        System.setProperty("rusted.enabled", "true");
        check(NativeCodec.tryCompress(new byte[512]) != null, "Opt-in native encoder not used");
        check(NativeCodec.encodedPackets() == 1, "Native packet counter not incremented");
        Deflater encoder = new Deflater(); byte[] scratch = new byte[8192];
        int cases = 0;
        try {
            Random random = new Random(42);
            for (int size : new int[]{0, 1, 255, 256, 257, 8192, 65536, 1048576, 8388608}) {
                for (boolean randomContent : new boolean[]{false, true}) {
                    byte[] bytes = new byte[size]; if (randomContent) random.nextBytes(bytes);
                    byte[] rust = NativeCodec.compressNative(bytes);
                    check(Arrays.equals(bytes, javaDecode(rust, size)), "Rust -> Java bytes differ at size " + size);
                    byte[] java = javaEncode(bytes, encoder, scratch);
                    check(Arrays.equals(bytes, NativeCodec.decompressNative(java, size)), "Java -> Rust bytes differ at size " + size);
                    cases++;
                }
            }
            byte[] valid = NativeCodec.compressNative(new byte[]{1, 2, 3});
            expectRejected(valid, -1); expectRejected(valid, 0); expectRejected(valid, 4);
            expectRejected(Arrays.copyOf(valid, valid.length - 1), 3);
            expectRejected(Arrays.copyOf(valid, valid.length + 1), 3);
            valid[valid.length - 1] ^= 1; expectRejected(valid, 3);
            expectRejected(new byte[1], NativeCodec.MAX_BYTES + 1);
            boolean oversized = false;
            try { NativeCodec.compressNative(new byte[NativeCodec.MAX_BYTES + 1]); } catch (IllegalArgumentException expected) { oversized = true; }
            check(oversized, "Native compressor accepted oversized input");
            var threads = Executors.newFixedThreadPool(4);
            try {
                List<Future<Boolean>> futures = new ArrayList<>();
                for (int thread = 0; thread < 4; thread++) {
                    final int number = thread;
                    futures.add(threads.submit(() -> {
                        for (int i = 0; i < 100; i++) {
                            byte[] input = new byte[1024 + number]; Arrays.fill(input, (byte) number);
                            if (!Arrays.equals(input, javaDecode(NativeCodec.compressNative(input), input.length))) return false;
                        }
                        return true;
                    }));
                }
                for (Future<Boolean> future : futures) check(future.get(), "Concurrent JNI state corruption");
            } finally { threads.shutdownNow(); }
        } finally { encoder.end(); }
        Files.createDirectories(report.toAbsolutePath().getParent());
        Files.writeString(report, "{\"scenario\":\"Java-zlib-native-Rust-interoperability\",\"passed\":true,\"payload_cases\":" + cases + ",\"concurrent_calls\":400,\"assertions\":" + assertions + ",\"game_integration_validated\":false}");
        System.out.println("PASS: " + cases + " payload cases, 400 concurrent JNI calls, " + assertions + " assertions; Minecraft integration NOT tested");
    }
}
