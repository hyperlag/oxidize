import java.net.URL;

public class URLTest {
    public static void main(String[] args) throws Exception {
        URL url = new URL("http://example.com:8080/path/to/resource?key=value&foo=bar#section");
        System.out.println("URL: " + url);
        System.out.println("Protocol: " + url.getProtocol());
        System.out.println("Host: " + url.getHost());
        System.out.println("Port: " + url.getPort());
        System.out.println("Path: " + url.getPath());
        System.out.println("Query: " + url.getQuery());
        System.out.println("Ref: " + url.getRef());
        System.out.println("File: " + url.getFile());
        System.out.println("DefaultPort: " + url.getDefaultPort());

        // URL without port
        URL url2 = new URL("https://www.example.com/index.html");
        System.out.println("Protocol2: " + url2.getProtocol());
        System.out.println("Host2: " + url2.getHost());
        System.out.println("Port2: " + url2.getPort());
        System.out.println("Path2: " + url2.getPath());
        System.out.println("DefaultPort2: " + url2.getDefaultPort());

        // Simple URL
        URL url3 = new URL("http://localhost/test");
        System.out.println("Host3: " + url3.getHost());
        System.out.println("Port3: " + url3.getPort());
        System.out.println("Path3: " + url3.getPath());

        // toString / toExternalForm
        System.out.println("toString: " + url.toString());
        System.out.println("toExternalForm: " + url.toExternalForm());
    }
}
