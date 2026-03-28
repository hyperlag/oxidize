import java.util.LinkedHashMap;

class LinkedHashMapBasic {
    public static void main(String[] args) {
        LinkedHashMap<String, Integer> map = new LinkedHashMap<>();
        map.put("cherry", 3);
        map.put("apple", 1);
        map.put("banana", 2);
        System.out.println(map.size());
        System.out.println(map.get("apple"));
        System.out.println(map.containsKey("banana"));
        map.remove("cherry");
        System.out.println(map.size());
        System.out.println(map.containsKey("cherry"));
    }
}
