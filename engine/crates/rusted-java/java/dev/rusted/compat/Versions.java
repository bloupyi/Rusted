// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package dev.rusted.compat;
import java.util.*;
import net.fabricmc.loader.api.*;

/** Numeric components and SemVer precedence, implemented from the public format contract. */
public final class Versions {
    public static Version parse(String value) throws VersionParsingException {
        if (value == null || value.isEmpty()) throw new VersionParsingException("Empty version");
        try { return semantic(value); }
        catch (VersionParsingException error) { return new Text(value); }
    }
    public static SemanticVersion semantic(String value) throws VersionParsingException {
        if (value == null || value.isEmpty()) throw new VersionParsingException("Empty version");
        // Fabric preserves build metadata verbatim; unlike prerelease identifiers,
        // it may contain characters outside strict SemVer (observed binary contract).
        String[] build = value.split("\\+", 2);
        int dash = build[0].indexOf('-');
        String core = dash < 0 ? build[0] : build[0].substring(0, dash);
        String pre = dash < 0 ? null : build[0].substring(dash + 1);
        if (pre != null && !pre.isEmpty() && !identifiers(pre)) throw new VersionParsingException(value);
        String[] components = core.split("\\.", -1);
        int[] numbers = new int[components.length];
        for (int i = 0; i < components.length; i++) {
            String component = components[i];
            if (!component.matches("[0-9]+")) throw new VersionParsingException(value);
            try { numbers[i] = Integer.parseInt(component); }
            catch (NumberFormatException error) { throw new VersionParsingException(value, error); }
        }
        String normalized = Arrays.stream(numbers).mapToObj(Integer::toString).collect(java.util.stream.Collectors.joining("."));
        if (pre != null) normalized += "-" + pre;
        if (build.length == 2) normalized += "+" + build[1];
        return new Numeric(normalized, numbers, pre, build.length == 2 ? build[1] : null);
    }
    private static boolean identifiers(String value) {
        return value.matches("[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*");
    }
    private record Text(String getFriendlyString) implements Version {
        public int compareTo(Version other) { return getFriendlyString.compareTo(other.getFriendlyString()); }
    }
    private record Numeric(String getFriendlyString, int[] components, String pre, String build) implements SemanticVersion {
        public boolean hasWildcard() { return false; }
        public int getVersionComponentCount() { return components.length; }
        public int getVersionComponent(int position) {
            if (position < 0) throw new IndexOutOfBoundsException(position);
            return position < components.length ? components[position] : 0;
        }
        public Optional<String> getPrereleaseKey() { return Optional.ofNullable(pre); }
        public Optional<String> getBuildKey() { return Optional.ofNullable(build); }
        public boolean equals(Object other) {
            if (!(other instanceof Numeric version) || !Objects.equals(pre, version.pre) || !Objects.equals(build, version.build)) return false;
            for (int i = 0; i < Math.max(components.length, version.components.length); i++)
                if (getVersionComponent(i) != version.getVersionComponent(i)) return false;
            return true;
        }
        public int hashCode() { return Objects.hash(Arrays.hashCode(components), pre, build); }
        public int compareTo(Version other) {
            if (!(other instanceof SemanticVersion version)) return getFriendlyString.compareTo(other.getFriendlyString());
            for (int i = 0; i < Math.max(components.length, version.getVersionComponentCount()); i++) {
                int a = getVersionComponent(i), b = version.getVersionComponent(i);
                if (a != COMPONENT_WILDCARD && b != COMPONENT_WILDCARD && a != b) return Integer.compare(a, b);
            }
            String right = version.getPrereleaseKey().orElse(null);
            if (Objects.equals(pre, right)) return 0;
            if (pre == null) return 1;
            if (right == null) return -1;
            String[] a = pre.split("\\.", -1), b = right.split("\\.", -1);
            for (int i = 0; i < Math.min(a.length, b.length); i++) {
                if (a[i].equals(b[i])) continue;
                boolean an = a[i].matches("[0-9]+"), bn = b[i].matches("[0-9]+");
                if (an && bn) return new java.math.BigInteger(a[i]).compareTo(new java.math.BigInteger(b[i]));
                if (an != bn) return an ? -1 : 1;
                return a[i].compareTo(b[i]);
            }
            return Integer.compare(a.length, b.length);
        }
    }
    private Versions() { }
}
