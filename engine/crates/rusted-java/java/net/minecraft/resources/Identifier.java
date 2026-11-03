// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.minecraft.resources;
import java.nio.file.Path;
import java.util.function.UnaryOperator;
import net.minecraft.IdentifierException;

/** Value portion of the 26.3 Identifier contract; serialization codecs are not implemented. */
public final class Identifier implements Comparable<Identifier> {
    public static final char NAMESPACE_SEPARATOR = ':';
    public static final String DEFAULT_NAMESPACE = "minecraft", REALMS_NAMESPACE = "realms";
    private final String namespace, path;
    private Identifier(String namespace, String path) { this.namespace = namespace; this.path = path; }
    public static Identifier fromNamespaceAndPath(String namespace, String path) {
        if (!isValidNamespace(namespace) || !isValidPath(path)) throw new IdentifierException("Invalid identifier: " + namespace + ":" + path);
        return new Identifier(namespace, path);
    }
    public static Identifier parse(String value) { return bySeparator(value, ':'); }
    public static Identifier withDefaultNamespace(String value) { return fromNamespaceAndPath(DEFAULT_NAMESPACE, value); }
    public static Identifier bySeparator(String value, char separator) {
        int colon = value.indexOf(separator);
        return colon < 0 ? withDefaultNamespace(value) : fromNamespaceAndPath(colon == 0 ? DEFAULT_NAMESPACE : value.substring(0, colon), value.substring(colon + 1));
    }
    public static Identifier tryParse(String value) { return tryBySeparator(value, ':'); }
    public static Identifier tryBySeparator(String value, char separator) {
        int colon = value.indexOf(separator);
        return colon < 0 ? tryBuild(DEFAULT_NAMESPACE, value) : tryBuild(colon == 0 ? DEFAULT_NAMESPACE : value.substring(0, colon), value.substring(colon + 1));
    }
    public static Identifier tryBuild(String namespace, String path) {
        return isValidNamespace(namespace) && isValidPath(path) ? new Identifier(namespace, path) : null;
    }
    public String getNamespace() { return namespace; }
    public String getPath() { return path; }
    public Identifier withPath(String value) { return fromNamespaceAndPath(namespace, value); }
    public Identifier withPath(UnaryOperator<String> operator) { return withPath(operator.apply(path)); }
    public Identifier withPrefix(String prefix) { return withPath(prefix + path); }
    public Identifier withSuffix(String suffix) { return withPath(path + suffix); }
    public String toString() { return namespace + ":" + path; }
    public boolean equals(Object other) { return other instanceof Identifier id && namespace.equals(id.namespace) && path.equals(id.path); }
    public int hashCode() { return 31 * namespace.hashCode() + path.hashCode(); }
    public int compareTo(Identifier other) { int order = path.compareTo(other.path); return order == 0 ? namespace.compareTo(other.namespace) : order; }
    public Path resolveAgainst(Path root) {
        Path resolved = root.resolve(namespace).resolve(path);
        if (!resolved.normalize().startsWith(root.normalize())) throw new IllegalStateException("Identifier escapes resource root: " + this);
        return resolved;
    }
    public String toDebugFileName() { return toString().replace('/', '_').replace(':', '_'); }
    public String toShortString() { return namespace.equals(DEFAULT_NAMESPACE) ? path : toString(); }
    public static boolean isAllowedInIdentifier(char c) { return c == ':' || validPathChar(c); }
    public static boolean validPathChar(char c) { return c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '_' || c == '-' || c == '.' || c == '/'; }
    public static boolean isValidPath(String value) { return value.chars().allMatch(c -> c <= Character.MAX_VALUE && validPathChar((char) c)); }
    public static boolean isValidNamespace(String value) { return value.chars().allMatch(c -> c != '/' && c <= Character.MAX_VALUE && validPathChar((char) c)); }
}
