import java.util.HashMap;
import java.util.Map;
import java.util.TreeMap;

class MapMutationApi {
    public static void main(String[] args) {
        // putIfAbsent — insert only when key absent
        TreeMap<String, Integer> m1 = new TreeMap<>();
        m1.put("Alice", 10);
        m1.putIfAbsent("Alice", 99); // already present, no change
        m1.putIfAbsent("Bob", 20);   // new entry
        System.out.println(m1.get("Alice")); // 10
        System.out.println(m1.get("Bob"));   // 20
        System.out.println(m1.size());        // 2

        // computeIfAbsent — compute and cache on first access
        HashMap<String, Integer> m2 = new HashMap<>();
        m2.computeIfAbsent("hello", k -> k.length()); // inserts 5
        m2.computeIfAbsent("hello", k -> 999);         // already present, no change
        System.out.println(m2.get("hello")); // 5

        // merge — accumulate word counts
        TreeMap<String, Integer> m3 = new TreeMap<>();
        m3.merge("apple",  1, (a, b) -> a + b);
        m3.merge("apple",  1, (a, b) -> a + b);
        m3.merge("apple",  1, (a, b) -> a + b);
        m3.merge("banana", 1, (a, b) -> a + b);
        System.out.println(m3.get("apple"));  // 3
        System.out.println(m3.get("banana")); // 1

        // forEach — iterate in sorted order (TreeMap)
        TreeMap<String, Integer> m4 = new TreeMap<>();
        m4.put("ant",    1);
        m4.put("bee",    2);
        m4.put("cat",    3);
        m4.forEach((k, v) -> System.out.println(k + ":" + v));

        // replace — overwrite existing value
        HashMap<String, Integer> m5 = new HashMap<>();
        m5.put("x", 42);
        m5.replace("x", 100);
        System.out.println(m5.get("x")); // 100

        // replaceAll — double every value (sorted output via TreeMap)
        TreeMap<String, Integer> m6 = new TreeMap<>();
        m6.put("a", 1);
        m6.put("b", 2);
        m6.put("c", 3);
        m6.replaceAll((k, v) -> v * 2);
        m6.forEach((k, v) -> System.out.println(k + "=" + v));
    }
}
