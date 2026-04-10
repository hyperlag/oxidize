import java.util.stream.IntStream;

public class IntStreamRange {
    public static void main(String[] args) {
        // half-open range [1, 6) = 1+2+3+4+5 = 15
        int sum = IntStream.range(1, 6).sum();
        System.out.println(sum);  // 15

        // closed range [1, 5] = 1+2+3+4+5 = 15
        int sum2 = IntStream.rangeClosed(1, 5).sum();
        System.out.println(sum2);  // 15

        // count
        long count = IntStream.range(0, 10).count();
        System.out.println(count);  // 10

        // filter then sum
        int odd = IntStream.range(1, 11).filter(x -> x % 2 != 0).sum();
        System.out.println(odd);  // 25 (1+3+5+7+9)
    }
}
