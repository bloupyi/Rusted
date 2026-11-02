# rusted-java

Experimental JVM host for existing Fabric JAR bytecode in the native Rusted
process. Rust retains simulation, networking and rendering. Opt-in only; see
[scope and validation](../../../docs/JAVA-MOD-BRIDGE.md).

## Original source provenance

Every Java file in `java/` is an original, deliberately small API facade.
No Minecraft, Fabric Loader, Fabric API or third-party mod implementation source
was read or copied to write these files. Package and method names describe the
binary contracts needed by an existing JAR. The reference inputs were public
Fabric API documentation, mod metadata and public usage documentation, execution
of the unchanged YOSBR 0.1.2 JAR, and the player's local Minecraft 26.3 bytecode
signatures queried with `javap`.

The public-tree audit permits only the exact listed facade source files. It
continues to reject other Java sources, compiled classes, game JARs and prepared
resources. `build.rs` compiles only our sources and embeds their API archive in
the Rust binary; it does not embed a Java Minecraft client or a third-party mod.

## Implemented subset

| API surface | Current behavior |
| --- | --- |
| `ModInitializer`, `ClientModInitializer`, `PreLaunchEntrypoint` | Call class-based initializers in phase order |
| `LanguageAdapter`, `ModContainer`, `ModMetadata` | Class constructors, adapter `create`, mod id/name |
| `FabricLoader` | Game/config paths, legacy File aliases, client environment, loaded-id query |
| `Event`, `ClientTickEvents` | Register callbacks and dispatch start/end client ticks |
| `Minecraft.getInstance`, `player`, `level` | Views updated from native state, null outside a world |
| `Minecraft.execute`, `isSameThread` | Client-thread execution or queued work |
| `LocalPlayer.getHealth`, `sendSystemMessage` | Native health snapshot and literal client chat |
| `Level`, `ClientLevel`, `LevelData` | Game-time snapshot, including the covariant 26.3 descriptor |
| `Component.literal`, `MutableComponent.getString` | Literal text only |

These are not complete implementations of the named APIs. Snapshot mutations do
not modify the Rust world. Missing game classes are rejected by the module class
loader instead of falling through to a Java game JAR. This class-loader rule is
not a security sandbox for Java code.

One JVM and one pinned library live for the native process lifetime. JNI uses a
permanent attachment for the client thread so `Thread.currentThread()` identity
survives ticks. A failing event listener disables that module's callbacks and
records a diagnostic. The host bounds native chat output and JNI local frames.
Reloading requires a process restart.
