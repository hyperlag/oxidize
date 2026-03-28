import java.util.LinkedHashSet;

class LinkedHashSetBasic {
    public static void main(String[] args) {
        LinkedHashSet<Integer> set = new LinkedHashSet<>();
        set.add(30);
        set.add(10);
        set.add(20);
        set.add(10);
        System.out.println(set.size());
        System.out.println(set.contains(10));
        for (int n : set) {
            System.out.println(n);
        }
    }
}
