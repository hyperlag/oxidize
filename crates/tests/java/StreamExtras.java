import java.util.stream.Stream;

public class StreamExtras {
    public static void main(String[] args) {
        // Stream.of + anyMatch
        boolean any = Stream.of(1, 2, 3, 4, 5).anyMatch(x -> x > 3);
        System.out.println(any);  // true

        // Stream.of + allMatch
        boolean all = Stream.of(2, 4, 6).allMatch(x -> x % 2 == 0);
        System.out.println(all);  // true

        // Stream.of + noneMatch
        boolean none = Stream.of(1, 3, 5).noneMatch(x -> x % 2 == 0);
        System.out.println(none);  // true

        // Stream.of + count
        long count = Stream.of("a", "b", "c").count();
        System.out.println(count);  // 3

        // peek — passes elements through, runs action as side-effect
        long peeked = Stream.of(10, 20, 30).peek(x -> {}).count();
        System.out.println(peeked);  // 3
    }
}
