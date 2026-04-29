import java.util.List;
import java.util.Map;
import java.util.Set;

public class ImmutableCollections {
    public static void main(String[] args) {
        // List.of(varargs)
        List<String> list = List.of("alpha", "beta", "gamma");
        System.out.println(list.size());        // 3
        System.out.println(list.contains("beta")); // true
        System.out.println(list.get(0));        // alpha

        // List.of() — zero-arg form
        List<Integer> empty = List.of();
        System.out.println(empty.size());       // 0

        // Set.of(varargs) — only test size and membership; no iteration (order undefined)
        Set<Integer> nums = Set.of(10, 20, 30);
        System.out.println(nums.size());        // 3
        System.out.println(nums.contains(20));  // true
        System.out.println(nums.contains(99));  // false

        // Map.of(k1,v1,...) — 2-pair form; use get() for deterministic output
        Map<String, Integer> m = Map.of("one", 1, "two", 2);
        System.out.println(m.size());           // 2
        System.out.println(m.get("one"));       // 1

        // Map.entry(key, value)
        Map.Entry<String, Integer> e = Map.entry("key", 42);
        System.out.println(e.getKey());         // key
        System.out.println(e.getValue());       // 42

        // Map.ofEntries(Map.entry(...), ...)
        Map<String, Integer> m2 = Map.ofEntries(
            Map.entry("x", 10),
            Map.entry("y", 20)
        );
        System.out.println(m2.size());          // 2
        System.out.println(m2.get("x"));        // 10

        // List.copyOf(collection)
        List<String> copy = List.copyOf(list);
        System.out.println(copy.size());        // 3
        System.out.println(copy.get(2));        // gamma
    }
}
