package dev.rusted.bootstrap;

import java.io.OutputStream;
import java.io.BufferedInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.loader.api.entrypoint.PreLaunchEntrypoint;

/** Original launcher glue only. Does not patch Minecraft or implement any mod API. */
public final class RustedBootstrap implements PreLaunchEntrypoint {

    @Override
    public void onPreLaunch() {
        FabricLoader loader = FabricLoader.getInstance();
        Path game = loader.getGameDir().toAbsolutePath().normalize();
        Path executable = game.resolve("rusted/rusted-launch.exe");
        Process child = null;
        Thread cleanup = null;
        try {
            child = new ProcessBuilder(executable.toString(), "--bootstrap")
                .directory(game.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT).start();
            final Process running = child;
            cleanup = new Thread(() -> running.destroy(), "Rusted-native-shutdown");
            Runtime.getRuntime().addShutdownHook(cleanup);
            String version = loader.getModContainer("minecraft").orElseThrow()
                .getMetadata().getVersion().getFriendlyString();
            Path java = Path.of(System.getProperty("java.home"), "bin",
                System.getProperty("os.name").startsWith("Windows") ? "java.exe" : "java");
            StringBuilder json = new StringBuilder("{\"schema\":1,\"parent_pid\":")
                .append(ProcessHandle.current().pid()).append(",\"java\":");
            string(json, java.toString());
            json.append(",\"minecraft_version\":"); string(json, version);
            json.append(",\"game_directory\":"); string(json, game.toString());
            json.append(",\"classpath\":"); string(json, System.getProperty("java.class.path"));
            json.append(",\"game_args\":[");
            String[] args = loader.getLaunchArguments(false);
            for (int i = 0; i < args.length; i++) {
                if (i > 0) json.append(',');
                string(json, args[i]);
            }
            json.append("]}");
            byte[] input = json.toString().getBytes(StandardCharsets.UTF_8);
            if (input.length > 2 * 1024 * 1024) throw new IllegalArgumentException("input limit");
            try (OutputStream stream = child.getOutputStream()) { stream.write(input); }
            BufferedInputStream output = new BufferedInputStream(child.getInputStream());
            StringBuilder control = new StringBuilder();
            int next;
            while ((next = output.read()) != -1 && next != '\n') {
                if (control.length() >= 64) throw new IllegalArgumentException("control limit");
                control.append((char)next);
            }
            boolean javaMode = control.toString().equals("RUSTED_BOOTSTRAP_JAVA");
            if (!javaMode && !control.toString().equals("RUSTED_BOOTSTRAP_NATIVE"))
                throw new IllegalArgumentException("invalid control frame");
            Thread forward = new Thread(() -> {
                try { output.transferTo(System.out); }
                catch (java.io.IOException ignored) { }
            }, "Rusted-native-output");
            forward.setDaemon(true);
            forward.start();
            int status = child.waitFor();
            forward.join(5000);
            Runtime.getRuntime().removeShutdownHook(cleanup);
            if (!javaMode || status != 0) System.exit(status);
        } catch (Throwable error) {
            if (child != null) child.destroyForcibly();
            if (cleanup != null) {
                try { Runtime.getRuntime().removeShutdownHook(cleanup); }
                catch (IllegalStateException ignored) { }
            }
            // Never print launcher arguments, JSON or exception payloads: they contain a session.
            System.err.println("Rusted : demarrage automatique impossible. Voir les messages Rusted precedents ; reimporter le pack complet si le runtime manque.");
            System.exit(1);
        }
    }

    private static void string(StringBuilder out, String value) {
        out.append('"');
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            switch (c) {
                case '"': out.append("\\\""); break;
                case '\\': out.append("\\\\"); break;
                case '\n': out.append("\\n"); break;
                case '\r': out.append("\\r"); break;
                case '\t': out.append("\\t"); break;
                default:
                    if (c < 32) out.append(String.format("\\u%04x", (int)c));
                    else out.append(c);
            }
        }
        out.append('"');
    }
}
