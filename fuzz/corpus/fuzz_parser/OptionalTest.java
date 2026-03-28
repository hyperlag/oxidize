import java.util.Optional;

public class OptionalTest {
    public static void main(String[] args) {
        Optional<String> opt = Optional.of("hello");
        System.out.println(opt.isPresent());
        System.out.println(opt.get());

        Optional<String> empty = Optional.empty();
        System.out.println(empty.isPresent());
        System.out.println(empty.orElse("default"));
    }
}
