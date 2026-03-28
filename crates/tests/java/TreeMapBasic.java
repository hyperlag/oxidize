import java.util.TreeMap;

class TreeMapBasic {
    public static void main(String[] args) {
        TreeMap<Integer, String> map = new TreeMap<>();
        map.put(30, "thirty");
        map.put(10, "ten");
        map.put(20, "twenty");
        System.out.println(map.size());
        System.out.println(map.get(10));
        System.out.println(map.containsKey(20));
        System.out.println(map.firstKey());
        System.out.println(map.lastKey());
        map.remove(20);
        System.out.println(map.size());
        System.out.println(map.containsKey(20));
    }
}
