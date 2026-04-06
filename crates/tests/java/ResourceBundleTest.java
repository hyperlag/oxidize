// ResourceBundleTest.java
// Tests ResourceBundle.getBundle and new PropertyResourceBundle(ByteArrayInputStream).
import java.util.ResourceBundle;
import java.util.PropertyResourceBundle;
import java.io.ByteArrayInputStream;

public class ResourceBundleTest {
    public static void main(String[] args) throws Exception {
        // Test PropertyResourceBundle loaded from a ByteArrayInputStream
        String props = "greeting=hello\nfarewell=goodbye\n# comment\ncount:42";
        byte[] bytes = props.getBytes("UTF-8");
        ByteArrayInputStream bais = new ByteArrayInputStream(bytes);
        PropertyResourceBundle bundle = new PropertyResourceBundle(bais);

        System.out.println(bundle.getString("greeting"));
        System.out.println(bundle.getString("farewell"));
        System.out.println(bundle.getString("count"));
        System.out.println(bundle.containsKey("greeting"));
        System.out.println(bundle.containsKey("missing"));
        System.out.println(bundle.getObject("farewell"));
    }
}
