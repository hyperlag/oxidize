import java.util.Arrays;
import java.util.List;
import java.util.stream.Collectors;

public class ThisMethodRef {
    private final String prefix;

    public ThisMethodRef(String prefix) {
        this.prefix = prefix;
    }

    public String greet(String s) {
        return prefix + s;
    }

    public void run() {
        List<String> names = Arrays.asList("Alice", "Bob", "Charlie");
        List<String> greetings = names.stream()
            .map(this::greet)
            .collect(Collectors.toList());

        for (String g : greetings) {
            System.out.println(g);
        }
        names.stream().forEach(System.err::println);
    }

    public static void main(String[] args) {
        new ThisMethodRef("Hello, ").run();
    }
}
