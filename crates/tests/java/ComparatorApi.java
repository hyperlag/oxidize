import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Collectors;

class ComparatorApi {
    public static void main(String[] args) {
        List<String> sorted = new ArrayList<>();
        sorted.add("banana");
        sorted.add("cherry");
        sorted.add("apple");
        sorted.add("date");

        // naturalOrder
        sorted.sort(Comparator.naturalOrder());
        System.out.println(sorted.get(0));  // apple

        // reverseOrder
        sorted.sort(Comparator.reverseOrder());
        System.out.println(sorted.get(0));  // date

        // comparing by length: shortest first (date=4, apple=5, banana/cherry=6)
        sorted.sort(Comparator.comparing(s -> s.length()));
        System.out.println(sorted.get(0));  // date

        // stream.sorted(Comparator.reverseOrder())
        List<String> words = new ArrayList<>();
        words.add("banana");
        words.add("cherry");
        words.add("apple");
        words.add("date");
        String first = words.stream()
            .sorted(Comparator.reverseOrder())
            .collect(Collectors.toList())
            .get(0);
        System.out.println(first);  // date

        // reversed() on naturalOrder: reverse-alphabetical order
        sorted.sort(Comparator.naturalOrder().reversed());
        System.out.println(sorted.get(0));  // date (from [apple, banana, cherry, date] reversed)

        // thenComparing: naturalOrder then by length as secondary key
        sorted.sort(Comparator.naturalOrder().thenComparing(s -> s.length()));
        System.out.println(sorted.get(0));  // apple
    }
}
