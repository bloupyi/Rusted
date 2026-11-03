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
of the unchanged YOSBR 0.1.2 and pinned render-mod JARs, and the player's local
Minecraft 26.3 bytecode signatures queried with `javap`. Value contracts are also
compared by running the same original calls against the unmodified reference
binaries; third-party loader implementation source is not inspected.

The public-tree audit permits only the exact listed facade source files. It
continues to reject other Java sources, compiled classes, game JARs and prepared
resources. `build.rs` compiles only our sources and embeds their API archive in
the Rust binary; it does not embed a Java Minecraft client or a third-party mod.

## Implemented subset

| API surface | Current behavior |
| --- | --- |
| `ModInitializer`, `ClientModInitializer`, `PreLaunchEntrypoint` | Call initializers in phase order |
| `LanguageAdapter`, `ModContainer`, `ModMetadata` | Class constructors, static field references, static/instance method references, adapter `create`, id/name/version, read-only archive roots and resource lookup |
| `FabricLoader` | Game/config paths, legacy File aliases, client environment, module inventory and loaded-id query |
| `Version`, `SemanticVersion` | Numeric/prerelease ordering, opaque versions, normalized components and preserved build metadata |
| `Identifier` | Validated namespace/path values, factories, equality, ordering and resource-root path resolution; no codecs |
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

The inventory lists actual registered modules and native built-ins, rather than
claiming that absent Fabric API JARs were loaded. Dedicated server initializer
types may be linked by client mod classes, but server entrypoints are not run.

The `mod_entrypoint_diagnostic` example is a developer component probe of real
published JARs. It deliberately does not certify mixin behavior or rendering and
must never replace the player's stricter discovery path.
