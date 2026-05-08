import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

class CollectorsExtra {
    public static void main(String[] args) {
        List<Integer> numbers = Arrays.asList(1, 2, 3, 4, 5, 6);

        // partitioningBy — split into even and odd
        Map<Boolean, List<Integer>> parts = numbers.stream()
                .collect(Collectors.partitioningBy(n -> n % 2 == 0));
        List<Integer> evens = parts.get(true);
        List<Integer> odds  = parts.get(false);
        System.out.println(evens.size());  // 3
        System.out.println(odds.size());   // 3
        System.out.println(evens.get(0));  // 2
        System.out.println(odds.get(0));   // 1

        // averagingInt
        double avg = numbers.stream()
                .collect(Collectors.averagingInt(n -> n));
        System.out.println(avg);  // 3.5

        // toUnmodifiableMap
        Map<Integer, Integer> squares = numbers.stream()
                .collect(Collectors.toUnmodifiableMap(n -> n, n -> n * n));
        System.out.println(squares.get(3));  // 9
        System.out.println(squares.get(5));  // 25

        // summarizingInt
        var stats = numbers.stream()
                .collect(Collectors.summarizingInt(n -> n));
        System.out.println(stats.getCount());    // 6
        System.out.println(stats.getSum());      // 21
        System.out.println(stats.getMin());      // 1
        System.out.println(stats.getMax());      // 6
    }
}
