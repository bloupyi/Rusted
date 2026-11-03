// Original Rusted API facade; no Mojang, Fabric Loader or mod implementation copied.
package net.fabricmc.fabric.api.event;
import java.lang.reflect.Proxy;
import java.util.concurrent.CopyOnWriteArrayList;
import dev.rusted.compat.Host;
/** Public register/invoker contract; callbacks operate on native-backed facades. */
public class Event<T> {
    private record Listener<T>(String owner, T callback) { }
    private final CopyOnWriteArrayList<Listener<T>> listeners = new CopyOnWriteArrayList<>();
    private final T invoker;
    public Event(Class<T> type) {
        invoker = type.cast(Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[]{type}, (proxy, method, args) -> {
            if (method.getDeclaringClass() == Object.class) {
                return switch (method.getName()) {
                    case "hashCode" -> System.identityHashCode(proxy);
                    case "equals" -> proxy == args[0];
                    case "toString" -> type.getName() + " (Rusted invoker)";
                    default -> throw new UnsupportedOperationException(method.getName());
                };
            }
            for (Listener<T> listener : listeners) {
                if (Host.failed(listener.owner())) continue;
                Host.owner.set(listener.owner());
                try { method.invoke(listener.callback(), args); }
                catch (Throwable error) { Host.fail(listener.owner(), error); }
                finally { Host.owner.remove(); }
            }
            return null;
        }));
    }
    public void register(T callback) { listeners.add(new Listener<>(Host.owner.get(), java.util.Objects.requireNonNull(callback))); }
    public T invoker() { return invoker; }
}
