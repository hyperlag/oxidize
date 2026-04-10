import java.util.Arrays;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

public class CollectorsVariants {
    public static void main(String[] args) {
        // toSet — deduplicates
        List<Integer> nums = Arrays.asList(1, 2, 2, 3);
        Set<Integer> s = nums.stream().collect(Collectors.toSet());
        System.out.println(s.size());  // 3

        // joining without separator
        List<String> words = Arrays.asList("a", "b", "c");
        String joined0 = words.stream().collect(Collectors.joining());
        System.out.println(joined0);  // abc

        // joining with separator
        String joined = words.stream().collect(Collectors.joining(", "));
        System.out.println(joined);  // a, b, c

        // joining with separator, prefix, suffix
        String wrapped = words.stream().collect(Collectors.joining(", ", "[", "]"));
        System.out.println(wrapped);  // [a, b, c]

        // counting
        long count = words.stream().collect(Collectors.counting());
        System.out.println(count);  // 3

        // toUnmodifiableList (same as toList)
        List<String> ul = words.stream().collect(Collectors.toUnmodifiableList());
        System.out.println(ul.size());  // 3
    }
}
