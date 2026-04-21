import java.util.List;
import java.util.Arrays;

public class MultiArgMethodRef {
    static int add(int a, int b) {
        return a + b;
    }

    static int pickMax(int a, int b) {
        return a > b ? a : b;
    }

    public static void main(String[] args) {
        List<Integer> nums = Arrays.asList(1, 2, 3, 4, 5);

        // Integer::sum — known binary static method ref
        int sum = nums.stream().reduce(0, Integer::sum);
        System.out.println(sum);

        // Integer::max — known binary static method ref
        int max = nums.stream().reduce(0, Integer::max);
        System.out.println(max);

        // Integer::min — known binary static method ref
        int min = nums.stream().reduce(100, Integer::min);
        System.out.println(min);

        // Math::max — known binary static method ref
        int mathMax = nums.stream().reduce(0, Math::max);
        System.out.println(mathMax);

        // Math::min — known binary static method ref
        int mathMin = nums.stream().reduce(100, Math::min);
        System.out.println(mathMin);

        // User-defined binary static method ref
        int sum2 = nums.stream().reduce(0, MultiArgMethodRef::add);
        System.out.println(sum2);

        // Another user-defined binary static method ref
        int max2 = nums.stream().reduce(0, MultiArgMethodRef::pickMax);
        System.out.println(max2);
    }
}
