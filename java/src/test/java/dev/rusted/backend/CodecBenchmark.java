package dev.rusted.backend;

import java.io.ByteArrayOutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Random;
import java.util.zip.Deflater;

/** Backend-only measurements, not a game benchmark or a claim of improvement. */
public final class CodecBenchmark {
    private static volatile int sink;
    private static byte[] javaEncode(byte[] data, Deflater encoder, byte[] buffer) {
        encoder.reset(); encoder.setInput(data); encoder.finish();
        var out = new ByteArrayOutputStream();
        while (!encoder.finished()) { int n = encoder.deflate(buffer); out.write(buffer, 0, n); }
        return out.toByteArray();
    }
    private static double percentile(long[] samples, int percentile) {
        Arrays.sort(samples);
        return samples[(samples.length - 1) * percentile / 100] / 1000.0;
    }
    public static void main(String[] args) throws Exception {
        if (!NativeCodec.available()) throw new IllegalStateException(NativeCodec.loadError());
        StringBuilder json = new StringBuilder("{\"scenario\":\"backend-zlib-level6-JNI-versus-reused-Java-Deflater\",\"whole_game\":false,\"warmup\":30,\"samples_per_backend\":100,\"cases\":[");
        int index = 0;
        for (int size : new int[]{256, 8192, 1048576}) {
            for (boolean random : new boolean[]{false, true}) {
                byte[] data = new byte[size]; if (random) new Random(42).nextBytes(data);
                Deflater encoder = new Deflater(); byte[] scratch = new byte[8192];
                try {
                    for (int i = 0; i < 30; i++) { sink = javaEncode(data, encoder, scratch).length; sink = NativeCodec.compressNative(data).length; }
                    long[] rust = new long[100], java = new long[100];
                    // Alternate order to reduce systematic drift. No input work is omitted.
                    for (int i = 0; i < 100; i++) {
                        if (i % 2 == 0) {
                            long start = System.nanoTime(); sink = NativeCodec.compressNative(data).length; rust[i] = System.nanoTime() - start;
                            start = System.nanoTime(); sink = javaEncode(data, encoder, scratch).length; java[i] = System.nanoTime() - start;
                        } else {
                            long start = System.nanoTime(); sink = javaEncode(data, encoder, scratch).length; java[i] = System.nanoTime() - start;
                            start = System.nanoTime(); sink = NativeCodec.compressNative(data).length; rust[i] = System.nanoTime() - start;
                        }
                    }
                    if (index++ != 0) json.append(',');
                    json.append("{\"bytes\":").append(size).append(",\"content\":\"").append(random ? "seed42-random" : "zeros").append("\",\"rust_us_p50\":").append(percentile(rust, 50))
                        .append(",\"rust_us_p95\":").append(percentile(rust, 95)).append(",\"java_us_p50\":").append(percentile(java, 50)).append(",\"java_us_p95\":").append(percentile(java, 95))
                        .append(",\"rust_compressed_bytes\":").append(NativeCodec.compressNative(data).length).append(",\"java_compressed_bytes\":").append(javaEncode(data, encoder, scratch).length).append('}');
                } finally { encoder.end(); }
            }
        }
        json.append("]}"); Path path = Path.of(args[0]); Files.createDirectories(path.toAbsolutePath().getParent()); Files.writeString(path, json.toString());
        System.out.println("Backend measurements saved to " + path + "; no game speedup established");
    }
}
