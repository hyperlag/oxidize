import java.util.HashMap;
import java.util.Map;
import java.util.TreeMap;
import java.util.LinkedHashMap;

public class MapIteration {
    public static void main(String[] args) {
        // Use TreeMap so iteration order is deterministic
        TreeMap<String, Integer> map = new TreeMap<>();
        map.put("alpha", 1);
        map.put("beta", 2);
        map.put("gamma", 3);

        // keySet iteration
        System.out.println("keys:");
        for (String key : map.keySet()) {
            System.out.println("  " + key);
        }

        // values iteration
        int sum = 0;
        for (int v : map.values()) {
            sum += v;
        }
        System.out.println("sum=" + sum);

        // entrySet iteration
        System.out.println("entries:");
        for (Map.Entry<String, Integer> entry : map.entrySet()) {
            System.out.println("  " + entry.getKey() + "=" + entry.getValue());
        }

        System.out.println("done");
    }
}
