import java.util.Objects;

public class ObjectsUtil {
    public static void main(String[] args) {
        String s = Objects.requireNonNull("hello");
        System.out.println(s);
        System.out.println(Objects.isNull(s));
        System.out.println(Objects.nonNull(s));
        System.out.println(Objects.equals("abc", "abc"));
        System.out.println(Objects.equals("abc", "xyz"));
    }
}
