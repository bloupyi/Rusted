// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.minecraft;
public class IdentifierException extends RuntimeException {
    public IdentifierException(String message) { super(message); }
    public IdentifierException(String message, Throwable cause) { super(message, cause); }
}
