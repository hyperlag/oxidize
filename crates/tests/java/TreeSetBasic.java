import java.util.TreeSet;

class TreeSetBasic {
    public static void main(String[] args) {
        TreeSet<Integer> set = new TreeSet<>();
        set.add(30);
        set.add(10);
        set.add(20);
        set.add(10);
        System.out.println(set.size());
        System.out.println(set.contains(20));
        System.out.println(set.first());
        System.out.println(set.last());
        for (int n : set) {
            System.out.println(n);
        }
    }
}
