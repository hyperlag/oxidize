import java.io.BufferedReader;
import java.io.InputStreamReader;

public class ProcessBuilderTest {
    public static void main(String[] args) throws Exception {
        ProcessBuilder pb = new ProcessBuilder("echo", "hello");
        Process p = pb.start();
        BufferedReader reader = new BufferedReader(new InputStreamReader(p.getInputStream()));
        String line = reader.readLine();
        int exit = p.waitFor();
        System.out.println("output=" + line);
        System.out.println("exit=" + exit);
    }
}
