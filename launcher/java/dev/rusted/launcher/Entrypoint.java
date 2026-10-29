package dev.rusted.launcher;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
/** Original glue: keeps Minecraft session arguments off the operating-system command line. */
public final class Entrypoint {
    public static void main(String[] argv) throws Exception {
        if (argv.length != 1) throw new IllegalArgumentException("Rusted launch frame missing");
        Path path=Path.of(argv[0]); byte[] bytes;
        try { bytes=Files.readAllBytes(path); } finally { Files.deleteIfExists(path); }
        if (bytes.length>2*1024*1024) throw new IllegalArgumentException("Rusted launch frame too large");
        try (DataInputStream input=new DataInputStream(new ByteArrayInputStream(bytes))) {
            String main=string(input);
            if (!main.equals("net.minecraft.client.main.Main") && !main.equals("net.fabricmc.loader.impl.launch.knot.KnotClient"))
                throw new IllegalArgumentException("Unsupported Rusted main class");
            int count=input.readInt();if(count<0||count>4096)throw new IllegalArgumentException("Rusted argument limit");
            String[] args=new String[count];for(int i=0;i<count;i++)args[i]=string(input);
            if(input.read()!=-1)throw new IllegalArgumentException("Unexpected launch data");
            Class.forName(main).getMethod("main",String[].class).invoke(null,(Object)args);
        }
    }
    private static String string(DataInputStream input) throws IOException {
        int size=input.readInt();if(size<0||size>131072)throw new IOException("Rusted string limit");
        byte[] bytes=input.readNBytes(size);if(bytes.length!=size)throw new EOFException();
        return new String(bytes,StandardCharsets.UTF_8);
    }
}
