import java.util.List;
import java.util.Arrays;
import java.util.stream.Collectors;

public class BoundMethodRef {
    private String prefix;

    public BoundMethodRef(String prefix) {
        this.prefix = prefix;
    }

    public String addPrefix(String s) {
        return prefix + s;
    }

    public static void main(String[] args) {
        BoundMethodRef greeter = new BoundMethodRef("Hello, ");
        List<String> names = Arrays.asList("Alice", "Bob", "Charlie");

        // Bound instance method reference: obj::method
        List<String> greetings = names.stream()
            .map(greeter::addPrefix)
            .collect(Collectors.toList());

        for (String g : greetings) {
            System.out.println(g);
        }

        // System.out::println — field-access bound method ref
        names.stream().forEach(System.out::println);
    }
}
