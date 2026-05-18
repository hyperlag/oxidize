import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import java.util.stream.Stream;

public class StreamEnhancements {
    public static void main(String[] args) {
        // stream.min(Comparator.naturalOrder())
        List<Integer> nums = Arrays.asList(3, 1, 4, 1, 5, 9, 2, 6);
        Optional<Integer> minVal = nums.stream().min(Comparator.naturalOrder());
        System.out.println(minVal.get());  // 1

        // stream.max(Comparator.naturalOrder())
        Optional<Integer> maxVal = nums.stream().max(Comparator.naturalOrder());
        System.out.println(maxVal.get());  // 9

        // stream.min/max with custom comparator (by string length)
        List<String> words = Arrays.asList("fig", "banana", "kiwi", "ap");
        Optional<String> shortest = words.stream().min(Comparator.comparingInt(s -> s.length()));
        System.out.println(shortest.get());  // ap

        Optional<String> longest = words.stream().max(Comparator.comparingInt(s -> s.length()));
        System.out.println(longest.get());  // banana

        // stream.toList() (Java 16+)
        List<Integer> doubled = nums.stream()
            .filter(n -> n > 3)
            .map(n -> n * 2)
            .toList();
        // Elements from nums > 3: 4,5,9,6 → doubled: 8,10,18,12
        for (int v : doubled) {
            System.out.println(v);
        }

        // stream.findAny() — returns some element from a non-empty stream
        Optional<String> found = words.stream().filter(s -> s.length() == 3).findAny();
        System.out.println(found.isPresent());  // true

        Optional<String> notFound = words.stream().filter(s -> s.length() == 99).findAny();
        System.out.println(notFound.isPresent());  // false

        // Stream.concat(s1, s2)
        Stream<String> s1 = Stream.of("a", "b");
        Stream<String> s2 = Stream.of("c", "d");
        Stream<String> combined = Stream.concat(s1, s2);
        combined.forEach(s -> System.out.println(s));  // a b c d
    }
}
