package dev.rusted.backend;

import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.Locale;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import java.util.zip.Inflater;

/** JNI bridge inside the original game's JVM. No game/UI/account replacement. */
public final class NativeCodec {
    public static final int MAX_BYTES = 8 * 1024 * 1024;
    private static final System.Logger LOG = System.getLogger("Rusted");
    private static final AtomicBoolean HEALTHY = new AtomicBoolean(false);
    private static final AtomicLong ENCODED_PACKETS = new AtomicLong();
    private static String loadError;

    static {
        try {
            String explicit = System.getProperty("rusted.native.path");
            if (explicit != null) {
                Path path = Path.of(explicit);
                if (!path.isAbsolute() || !Files.isRegularFile(path)) throw new IllegalArgumentException("Native path must be an absolute regular file");
                System.load(path.toString());
            } else {
                String os = System.getProperty("os.name").toLowerCase(Locale.ROOT);
                String arch = System.getProperty("os.arch");
                if (!arch.equals("amd64") && !arch.equals("x86_64")) throw new UnsupportedOperationException("Only x86_64 currently packaged");
                String platform = os.contains("win") ? "windows-x86_64" : os.contains("linux") ? "linux-x86_64" : "unsupported";
                String name = os.contains("win") ? "rusted_backend.dll" : "librusted_backend.so";
                String resource = "/native/" + platform + "/" + name;
                byte[] binary;
                String expected;
                try (InputStream stream = NativeCodec.class.getResourceAsStream(resource);
                     InputStream hash = NativeCodec.class.getResourceAsStream(resource + ".sha256")) {
                    if (stream == null || hash == null) throw new IllegalStateException("No packaged native library for " + platform);
                    binary = stream.readNBytes(32 * 1024 * 1024 + 1);
                    if (binary.length > 32 * 1024 * 1024) throw new IllegalStateException("Native library too large");
                    expected = new String(hash.readNBytes(128), java.nio.charset.StandardCharsets.US_ASCII).trim();
                }
                String actual = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(binary));
                if (!actual.equals(expected)) throw new SecurityException("Packaged native SHA-256 mismatch");
                Path dir = Files.createTempDirectory("rusted-native-");
                Path path = dir.resolve(name);
                Files.write(path, binary);
                dir.toFile().deleteOnExit();
                path.toFile().deleteOnExit();
                System.load(path.toAbsolutePath().toString());
            }
            if (abiVersion() != 1) throw new IllegalStateException("Native ABI mismatch");
            byte[] probe = "Rusted JNI startup self-test".getBytes(java.nio.charset.StandardCharsets.UTF_8);
            Inflater inflater = new Inflater();
            try {
                inflater.setInput(compressNative(probe));
                byte[] result = new byte[probe.length + 1];
                int length = inflater.inflate(result);
                if (!inflater.finished() || length != probe.length || !Arrays.equals(probe, Arrays.copyOf(result, length))) throw new IllegalStateException("Native zlib self-test failed");
            } finally { inflater.end(); }
            HEALTHY.set(true);
            LOG.log(System.Logger.Level.INFO, "Rust backend JNI loaded; network encode enabled=" + Boolean.getBoolean("rusted.enabled"));
        } catch (Exception | LinkageError error) {
            loadError = error.toString();
            LOG.log(System.Logger.Level.WARNING, "Rust backend unavailable; keeping Minecraft Java encode: " + loadError);
        }
    }

    private NativeCodec() { }
    public static boolean available() { return HEALTHY.get(); }
    public static String loadError() { return loadError; }
    public static long encodedPackets() { return ENCODED_PACKETS.get(); }
    public static boolean enabled() { return Boolean.getBoolean("rusted.enabled") && available(); }
    /** Returns null BEFORE modifying Netty buffers when the original Java path is needed. */
    public static byte[] tryCompress(byte[] input) {
        if (!enabled() || input.length > MAX_BYTES) return null;
        try {
            byte[] output = compressNative(input);
            long count = ENCODED_PACKETS.incrementAndGet();
            if (count == 1) LOG.log(System.Logger.Level.INFO, "First native encode call; Minecraft network hook still requires separate verification");
            return output;
        } catch (RuntimeException | LinkageError error) {
            HEALTHY.set(false);
            LOG.log(System.Logger.Level.WARNING, "Native encoder disabled; Java fallback: " + error);
            return null;
        }
    }
    static native int abiVersion();
    static native byte[] compressNative(byte[] input);
    static native byte[] decompressNative(byte[] input, int expected);
}
