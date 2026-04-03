import java.util.Timer;
import java.util.TimerTask;

class CountTask extends TimerTask {
    static int count = 0;
    public void run() {
        count++;
    }
}

class TimerTest {
    public static void main(String[] args) throws Exception {
        Timer timer = new Timer();
        CountTask task = new CountTask();
        // delay=0, period=100ms
        timer.schedule(task, 0, 100);
        Thread.sleep(550);
        timer.cancel();
        Thread.sleep(150);
        System.out.println("count >= 4: " + (CountTask.count >= 4));
        System.out.println("done");
    }
}
