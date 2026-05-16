import java.util.ArrayList;
import java.util.LinkedList;
import java.util.List;
import java.util.HashMap;
import java.util.Map;
import java.util.HashSet;
import java.util.Set;

public class ForEachLambda {
    public static void main(String[] args) {
        // List.forEach with single-arg lambda
        List<String> names = new ArrayList<>();
        names.add("Alice");
        names.add("Bob");
        names.add("Carol");
        names.forEach(name -> System.out.println("Hello, " + name));

        // List.forEach with multi-statement block lambda
        List<Integer> nums = new ArrayList<>();
        nums.add(1);
        nums.add(2);
        nums.add(3);
        nums.forEach(n -> {
            int doubled = n * 2;
            System.out.println(doubled);
        });

        // Map.forEach with two-argument BiConsumer lambda
        Map<String, Integer> scores = new HashMap<>();
        scores.put("Alice", 90);
        scores.put("Bob", 85);
        // Use TreeMap-sorted order by iterating a sorted list of keys
        List<String> keys = new ArrayList<>(scores.keySet());
        keys.sort((a, b) -> a.compareTo(b));
        keys.forEach(k -> System.out.println(k + "=" + scores.get(k)));

        // LinkedList.forEach
        LinkedList<String> ll = new LinkedList<>();
        ll.add("x");
        ll.add("y");
        ll.forEach(s -> System.out.println(s.toUpperCase()));
    }
}
