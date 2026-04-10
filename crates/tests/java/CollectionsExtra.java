import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

public class CollectionsExtra {
    public static void main(String[] args) {
        List<Integer> list = new ArrayList<>();
        list.add(3);
        list.add(1);
        list.add(4);
        list.add(1);
        list.add(5);

        System.out.println(Collections.min(list));
        System.out.println(Collections.max(list));
        System.out.println(Collections.frequency(list, 1));

        List<Integer> copies = Collections.nCopies(3, 9);
        System.out.println(copies.size());
        System.out.println(copies.get(0));
    }
}
