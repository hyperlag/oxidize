import java.util.EnumSet;

public class EnumSetBasic {
    enum Color { RED, GREEN, BLUE }

    public static void main(String[] args) {
        EnumSet<Color> set = EnumSet.noneOf(Color.class);
        set.add(Color.RED);
        set.add(Color.GREEN);
        System.out.println(set.size());
        System.out.println(set.contains(Color.RED));
        System.out.println(set.contains(Color.BLUE));
        set.remove(Color.RED);
        System.out.println(set.size());
        System.out.println(set.isEmpty());

        EnumSet<Color> set2 = EnumSet.of(Color.RED, Color.BLUE);
        System.out.println(set2.size());
        System.out.println(set2.contains(Color.RED));
        System.out.println(set2.contains(Color.GREEN));
    }
}
