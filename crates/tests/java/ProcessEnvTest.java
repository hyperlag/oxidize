import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.File;

public class ProcessEnvTest {
    public static void main(String[] args) throws Exception {
        ProcessBuilder pb = new ProcessBuilder("pwd");
        pb.directory(new File("/tmp"));
        Process p = pb.start();
        BufferedReader reader = new BufferedReader(new InputStreamReader(p.getInputStream()));
        String line = reader.readLine();
        int exit = p.waitFor();
        System.out.println("wd=" + line);
        System.out.println("exit=" + exit);
    }
}
