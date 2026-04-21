// Stage 21: this.wait() / this.notify() / this.notifyAll() inside
// synchronized methods and synchronized(this) blocks.
class WaitTask implements Runnable {
    private boolean useBlock;

    public WaitTask(boolean useBlock) {
        this.useBlock = useBlock;
    }

    public void run() {
        try {
            SynchronizedWaitNotify s = new SynchronizedWaitNotify();
            if (useBlock) {
                s.waitFlagBlockShared();
            } else {
                s.waitFlagMethodShared();
            }
        } catch (InterruptedException e) {
            throw new RuntimeException(e);
        }
    }
}

class SynchronizedWaitNotify {
    private static boolean methodFlag = false;
    private static boolean methodWaiting = false;
    private static boolean blockFlag = false;
    private static boolean blockWaiting = false;
    private static final SynchronizedWaitNotify METHOD_SHARED = new SynchronizedWaitNotify();
    private static final SynchronizedWaitNotify BLOCK_SHARED = new SynchronizedWaitNotify();

    // --- synchronized methods ---
    public synchronized void setFlagMethod() {
        methodFlag = true;
        this.notifyAll();           // this.notifyAll() in synchronized method
    }

    public synchronized void waitFlagMethod() throws InterruptedException {
        methodWaiting = true;
        this.notifyAll();
        while (!methodFlag) this.wait();  // this.wait() in synchronized method
    }

    public synchronized void awaitMethodWaiterReady() throws InterruptedException {
        while (!methodWaiting) this.wait();
    }

    public void waitFlagMethodShared() throws InterruptedException {
        METHOD_SHARED.waitFlagMethod();
    }

    public void awaitMethodWaiterReadyShared() throws InterruptedException {
        METHOD_SHARED.awaitMethodWaiterReady();
    }

    public void setFlagMethodShared() {
        METHOD_SHARED.setFlagMethod();
    }

    // --- synchronized(this) blocks ---
    public void setFlagBlock() {
        synchronized (this) {
            blockFlag = true;
            this.notifyAll();       // this.notifyAll() in synchronized(this)
        }
    }

    public void waitFlagBlock() throws InterruptedException {
        synchronized (this) {
            blockWaiting = true;
            this.notifyAll();
            while (!blockFlag) this.wait(); // this.wait() in synchronized(this)
        }
    }

    public void awaitBlockWaiterReady() throws InterruptedException {
        synchronized (this) {
            while (!blockWaiting) this.wait();
        }
    }

    public void waitFlagBlockShared() throws InterruptedException {
        BLOCK_SHARED.waitFlagBlock();
    }

    public void awaitBlockWaiterReadyShared() throws InterruptedException {
        BLOCK_SHARED.awaitBlockWaiterReady();
    }

    public void setFlagBlockShared() {
        BLOCK_SHARED.setFlagBlock();
    }

    public static void main(String[] args) throws InterruptedException {
        SynchronizedWaitNotify controller = new SynchronizedWaitNotify();
        // Test 1: this.wait()/this.notifyAll() in synchronized methods
        methodFlag = false;
        methodWaiting = false;
        Thread t1 = new Thread(new WaitTask(false));
        t1.start();
        controller.awaitMethodWaiterReadyShared();
        controller.setFlagMethodShared();
        t1.join();
        System.out.println("method ok");

        // Test 2: this.wait()/this.notifyAll() in synchronized(this) blocks
        blockFlag = false;
        blockWaiting = false;
        Thread t2 = new Thread(new WaitTask(true));
        t2.start();
        controller.awaitBlockWaiterReadyShared();
        controller.setFlagBlockShared();
        t2.join();
        System.out.println("block ok");
    }
}
