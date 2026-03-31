import java.util.concurrent.ConcurrentHashMap;

class ConcurrentHashMapTest {
    public static void main(String[] args) {
        ConcurrentHashMap<String, Integer> map = new ConcurrentHashMap<>();

        // put and get
        map.put("a", 1);
        map.put("b", 2);
        map.put("c", 3);
        System.out.println("size = " + map.size());

        // containsKey
        System.out.println("has a = " + map.containsKey("a"));
        System.out.println("has z = " + map.containsKey("z"));

        // putIfAbsent
        map.putIfAbsent("a", 99);
        map.putIfAbsent("d", 4);
        System.out.println("size after putIfAbsent = " + map.size());

        // getOrDefault
        int val = map.getOrDefault("z", -1);
        System.out.println("getOrDefault z = " + val);

        // remove
        map.remove("b");
        System.out.println("size after remove = " + map.size());

        // isEmpty
        System.out.println("isEmpty = " + map.isEmpty());

        // clear
        map.clear();
        System.out.println("size after clear = " + map.size());
        System.out.println("isEmpty after clear = " + map.isEmpty());
    }
}
