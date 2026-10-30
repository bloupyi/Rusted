# Known issues

Every game release lists the bullets below under "Known issues" (tools/release_train.py). Keep them short and
written for players; remove a bullet in the pull request that fixes it.

- Java code mods (including Sodium, Iris and Distant Horizons) require their JVM loader or a native port.
  Passive Fabric/Quilt resources and supported data, and dependency-free lowcode Forge/NeoForge modules,
  can load natively; this does not implement those loaders' Java APIs.
- Native shader-pack readers support a subset of shader features. An Iris JAR is not a native shader pack.
- The custom-item/resource-pack fixes have local regression coverage; Hypixel SkyBlock still needs an
  authenticated gameplay check. Resource loading time depends on the pack, storage and graphics device.
