import java.util.List;
import java.util.Arrays;

public class MethodReference {
    static void print(String s) {
        System.out.println(s);
    }

    public static void main(String[] args) {
        List<String> items = Arrays.asList("alpha", "beta", "gamma");
        items.stream().forEach(MethodReference::print);
    }
}
